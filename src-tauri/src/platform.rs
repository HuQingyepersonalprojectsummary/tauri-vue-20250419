use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::domain::{
    prefix_to_subnet_mask, validate_dns_combination, validate_doh_config,
    validate_gateway_in_subnet, validate_ipv4, validate_ipv6, validate_ipv6_dns_combination,
    validate_ipv6_prefix, validate_subnet_mask, verify_snapshot_restored, AdapterInfo,
    AdapterSnapshot, DohConfig, DohServerSetting, Ipv4AddressConfig, Ipv4Config, OperationResult,
};

/// 进程执行结果封装
pub struct ProcessOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// 获取 Windows 原生受信任的 System32 路径，彻底防止通过进程环境变量 (如 SystemRoot) 实施劫持 (A-12, R-06, F-03)
#[cfg(windows)]
fn get_trusted_system_directory() -> PathBuf {
    use std::os::windows::ffi::OsStringExt;
    extern "system" {
        fn GetSystemDirectoryW(lpBuffer: *mut u16, uSize: u32) -> u32;
    }
    let mut buf = [0u16; 260];
    let len = unsafe { GetSystemDirectoryW(buf.as_mut_ptr(), buf.len() as u32) };
    if len > 0 && (len as usize) < buf.len() {
        let os_str = std::ffi::OsString::from_wide(&buf[..len as usize]);
        PathBuf::from(os_str)
    } else {
        PathBuf::from("C:\\Windows\\System32")
    }
}

#[cfg(not(windows))]
fn get_trusted_system_directory() -> PathBuf {
    PathBuf::from("/bin")
}

/// 定位 Windows 可信系统工具路径，防止 PATH 环境变量劫持 (A-12, R-06, F-03)
pub fn get_system_binary(binary_name: &str) -> PathBuf {
    let sys32 = get_trusted_system_directory();

    let candidate = if binary_name.eq_ignore_ascii_case("powershell.exe") {
        sys32
            .join("WindowsPowerShell")
            .join("v1.0")
            .join("powershell.exe")
    } else {
        sys32.join(binary_name)
    };

    if candidate.is_absolute() && candidate.exists() {
        return candidate;
    }

    // 严格闭环 (Fail-Closed)：强制指向可信的标准绝对路径
    let hardcoded_sys32 = Path::new("C:\\Windows\\System32");
    if binary_name.eq_ignore_ascii_case("powershell.exe") {
        hardcoded_sys32
            .join("WindowsPowerShell")
            .join("v1.0")
            .join("powershell.exe")
    } else {
        hardcoded_sys32.join(binary_name)
    }
}

/// 解码 Windows 控制台输出（先试 UTF-8，再回退 GBK，防止中文系统乱码）
fn decode_output_bytes(bytes: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    let (cow, _, _) = encoding_rs::GBK.decode(bytes);
    cow.to_string()
}

const MAX_OUTPUT_BYTES: usize = 2 * 1024 * 1024; // 2MB 上限，防止无限管道缓冲引发 OOM

#[cfg(windows)]
mod job_control {
    use std::ffi::c_void;
    use std::os::windows::io::AsRawHandle;
    use std::ptr::null_mut;

    type RawHandle = *mut c_void;

    const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x00002000;
    const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS: u32 = 9;

    #[repr(C)]
    struct IO_COUNTERS {
        read_operation_count: u64,
        write_operation_count: u64,
        other_operation_count: u64,
        read_transfer_count: u64,
        write_transfer_count: u64,
        other_transfer_count: u64,
    }

    #[repr(C)]
    struct JOBOBJECT_BASIC_LIMIT_INFORMATION {
        per_process_user_time_limit: i64,
        per_job_user_time_limit: i64,
        limit_flags: u32,
        minimum_working_set_size: usize,
        maximum_working_set_size: usize,
        active_process_limit: u32,
        affinity: usize,
        priority_class: u32,
        scheduling_class: u32,
    }

    #[repr(C)]
    struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
        basic_limit_information: JOBOBJECT_BASIC_LIMIT_INFORMATION,
        io_info: IO_COUNTERS,
        process_memory_limit: usize,
        job_memory_limit: usize,
        peak_process_memory_limit: usize,
        peak_job_memory_limit: usize,
    }

    extern "system" {
        fn CreateJobObjectW(lpJobAttributes: *mut c_void, lpName: *const u16) -> RawHandle;
        fn SetInformationJobObject(
            hJob: RawHandle,
            JobObjectInformationClass: u32,
            lpJobObjectInformation: *const c_void,
            cbJobObjectInformationLength: u32,
        ) -> i32;
        fn AssignProcessToJobObject(hJob: RawHandle, hProcess: RawHandle) -> i32;
        fn TerminateJobObject(hJob: RawHandle, uExitCode: u32) -> i32;
        fn CloseHandle(hObject: RawHandle) -> i32;
    }

    pub struct ProcessJobGuard {
        job_handle: RawHandle,
    }

    impl ProcessJobGuard {
        pub fn new() -> Option<Self> {
            let job = unsafe { CreateJobObjectW(null_mut(), null_mut()) };
            if job.is_null() {
                return None;
            }

            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
            info.basic_limit_information.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

            let ok = unsafe {
                SetInformationJobObject(
                    job,
                    JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS,
                    &info as *const _ as *const c_void,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
            };

            if ok == 0 {
                unsafe { CloseHandle(job) };
                return None;
            }

            Some(ProcessJobGuard { job_handle: job })
        }

        pub fn assign_child(&self, child: &std::process::Child) -> bool {
            let proc_handle = child.as_raw_handle();
            let ok = unsafe { AssignProcessToJobObject(self.job_handle, proc_handle as RawHandle) };
            ok != 0
        }

        pub fn terminate(&self) {
            if !self.job_handle.is_null() {
                unsafe {
                    TerminateJobObject(self.job_handle, 1);
                }
            }
        }
    }

    impl Drop for ProcessJobGuard {
        fn drop(&mut self) {
            if !self.job_handle.is_null() {
                unsafe {
                    CloseHandle(self.job_handle);
                }
            }
        }
    }
}

/// 安全运行系统子进程 (A-04, R-05, F-04)
///
/// 核心特性：
/// 1. 静默无窗口执行：通过 Windows 原生标志位 `CREATE_NO_WINDOW (0x08000000)`，彻底杜绝控制台黑框与终端闪烁；
/// 2. 进程树生命周期管理：通过 Windows Job Object 将子进程及其派生的所有孙进程绑定，主进程终止或超时时强制一网打尽，绝无孤儿进程残留；
/// 3. 安全管道传输：支持通过 stdin 安全传递结构化参数，杜绝动态字符串拼接引起的注入隐患；
/// 4. 异步超时回收与容量保护：设置上限 2MB 缓冲区与精准计时器，防止管道阻塞或无限挂起引发的 OOM。
pub fn run_command_with_timeout(
    program: &Path,
    args: &[&str],
    stdin_data: Option<&[u8]>,
    timeout: Duration,
) -> Result<ProcessOutput, String> {
    #[cfg(windows)]
    let job_guard = job_control::ProcessJobGuard::new();

    let mut cmd = Command::new(program);
    cmd.args(args);
    cmd.stdin(if stdin_data.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Windows API 创建标志: CREATE_NO_WINDOW (0x08000000)
        // 创建无控制台窗口的后台进程，彻底消除打开应用或修改网络时的黑框闪烁，且零额外系统开销
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("启动程序 '{}' 失败: {}", program.display(), e))?;

    #[cfg(windows)]
    if let Some(ref jg) = job_guard {
        jg.assign_child(&child);
    }

    macro_rules! terminate_process_tree {
        ($c:expr) => {
            #[cfg(windows)]
            if let Some(ref jg) = job_guard {
                jg.terminate();
            }
            let _ = $c.kill();
            let _ = $c.wait();
        };
    }

    // 写入 stdin 并安全关闭
    if let Some(mut stdin) = child.stdin.take() {
        if let Some(data) = stdin_data {
            let data = data.to_vec();
            std::thread::spawn(move || {
                let _ = stdin.write_all(&data);
                let _ = stdin.flush();
                drop(stdin);
            });
        }
    }

    // 并发读取 stdout 和 stderr，带容量保护与 channel 通知
    let mut stdout_handle = child.stdout.take().unwrap();
    let mut stderr_handle = child.stderr.take().unwrap();

    let (stdout_tx, stdout_rx) = std::sync::mpsc::channel();
    let (stderr_tx, stderr_rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            match stdout_handle.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    if buf.len() + n <= MAX_OUTPUT_BYTES {
                        buf.extend_from_slice(&chunk[..n]);
                    } else if buf.len() < MAX_OUTPUT_BYTES {
                        let take = MAX_OUTPUT_BYTES - buf.len();
                        buf.extend_from_slice(&chunk[..take]);
                    }
                }
                Err(_) => break,
            }
        }
        let _ = stdout_tx.send(buf);
    });

    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            match stderr_handle.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    if buf.len() + n <= MAX_OUTPUT_BYTES {
                        buf.extend_from_slice(&chunk[..n]);
                    } else if buf.len() < MAX_OUTPUT_BYTES {
                        let take = MAX_OUTPUT_BYTES - buf.len();
                        buf.extend_from_slice(&chunk[..take]);
                    }
                }
                Err(_) => break,
            }
        }
        let _ = stderr_tx.send(buf);
    });

    // 轮询子进程状态并计算超时
    let start = Instant::now();
    let exit_status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if start.elapsed() >= timeout {
                    terminate_process_tree!(child);
                    return Err(format!(
                        "执行程序 '{}' 超时 ({} 毫秒)，已彻底终止子进程及所有后代进程",
                        program.display(),
                        timeout.as_millis()
                    ));
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(e) => {
                terminate_process_tree!(child);
                return Err(format!("等待子进程失败: {}", e));
            }
        }
    };

    // 在总超时预算内等待输出通道返回，彻底杜绝后代进程持有管道句柄导致死锁 (R-05)
    let remaining_stdout = timeout.saturating_sub(start.elapsed());
    let stdout_bytes = match stdout_rx.recv_timeout(remaining_stdout) {
        Ok(b) => b,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            terminate_process_tree!(child);
            return Err(format!(
                "读取程序 '{}' 标准输出超时 (总耗时超过 {} 毫秒)，已彻底终止进程树",
                program.display(),
                timeout.as_millis()
            ));
        }
        Err(_) => Vec::new(),
    };

    let remaining_stderr = timeout.saturating_sub(start.elapsed());
    let stderr_bytes = match stderr_rx.recv_timeout(remaining_stderr) {
        Ok(b) => b,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            terminate_process_tree!(child);
            return Err(format!(
                "读取程序 '{}' 标准错误输出超时 (总耗时超过 {} 毫秒)，已彻底终止进程树",
                program.display(),
                timeout.as_millis()
            ));
        }
        Err(_) => Vec::new(),
    };

    Ok(ProcessOutput {
        success: exit_status.success(),
        stdout: decode_output_bytes(&stdout_bytes),
        stderr: decode_output_bytes(&stderr_bytes),
    })
}

// 固定 PowerShell 脚本，不拼接任何动态用户输入，彻底免疫注入 (A-01)
const LIST_ADAPTERS_PS_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Get-NetAdapter | Select-Object @{Name='Name';Expression={$_.Name}}, @{Name='InterfaceIndex';Expression={$_.InterfaceIndex}}, @{Name='InterfaceGuid';Expression={$_.InterfaceGuid}}, @{Name='Status';Expression={$_.Status}}, @{Name='DisplayName';Expression={$_.InterfaceDescription}}, @{Name='MacAddress';Expression={$_.MacAddress}} | ConvertTo-Json -Compress
"#;

const GET_SNAPSHOT_PS_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
[Console]::InputEncoding = [System.Text.Encoding]::UTF8
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$raw = [Console]::In.ReadToEnd()
$req = $raw | ConvertFrom-Json
$target = [string]$req.target

$adapter = Get-NetAdapter | Where-Object { $_.Name -eq $target -or $_.InterfaceGuid -eq $target -or [string]$_.InterfaceIndex -eq $target } | Select-Object -First 1
if (-not $adapter) {
    throw "未找到匹配的网络适配器: $target"
}

$ipAddresses = @()
try {
    $ipAddresses = @(Get-NetIPAddress -InterfaceIndex $adapter.InterfaceIndex -AddressFamily IPv4 | Select-Object IPAddress, PrefixLength)
} catch {
    if ($_.Exception.Message -notmatch 'No matching|\u627e\u4e0d\u5230|\u672a\u627e\u5230') {
        throw "查询适配器 IP 异常: $_"
    }
}

$gateways = @()
try {
    $gateways = @(Get-NetRoute -InterfaceIndex $adapter.InterfaceIndex -DestinationPrefix '0.0.0.0/0' -AddressFamily IPv4 | Select-Object -ExpandProperty NextHop)
} catch {
    if ($_.Exception.Message -notmatch 'No matching|\u627e\u4e0d\u5230|\u672a\u627e\u5230') {
        throw "查询默认路由网关异常: $_"
    }
}

$dns = @()
try {
    $dns = @(Get-DnsClientServerAddress -InterfaceIndex $adapter.InterfaceIndex -AddressFamily IPv4 | Select-Object -ExpandProperty ServerAddresses)
} catch {
    throw "查询 DNS 服务器异常: $_"
}

$dhcp = $false
try {
    $ipIf = Get-NetIPInterface -InterfaceIndex $adapter.InterfaceIndex -AddressFamily IPv4
    $dhcp = ($ipIf.Dhcp -eq 'Enabled')
} catch {
    throw "查询 DHCP 状态异常: $_"
}

# 判定 DNS 是否由 DHCP 分配（查询注册表 NameServer 静态服务器）
$dnsDhcpEnabled = $true
$regPath = "HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\$($adapter.InterfaceGuid)"
if (Test-Path $regPath) {
    try {
        $reg = Get-ItemProperty -Path $regPath -ErrorAction Stop
        if ($reg -and $reg.NameServer -and [string]$reg.NameServer.Trim().Length -gt 0) {
            $dnsDhcpEnabled = $false
        }
    } catch {
        throw "读取网络适配器 DNS 注册表模式异常: $_"
    }
} else {
    $dnsDhcpEnabled = $dhcp
}

# 查询 IPv6 绑定状态 (通过精确网卡对象匹配，避免通配符展开，N-06)
$ipv6Enabled = $true
try {
    $binding = Get-NetAdapterBinding -ComponentID ms_tcpip6 -ErrorAction Stop | Where-Object { $_.Name -eq $adapter.Name }
    if ($binding) {
        $ipv6Enabled = [bool](@($binding)[0].Enabled)
    } else {
        $ipv6Enabled = $false
    }
} catch {
    throw "查询适配器 IPv6 绑定状态异常: $_"
}

# 查询全局与系统已配置的 DoH 服务器 (N-02, N-05, N-09)
$dohSettings = @{}
$hasDohCmdlet = $null -ne (Get-Command Get-DnsClientDohServerAddress -ErrorAction SilentlyContinue)
if ($hasDohCmdlet) {
    try {
        $allDoh = @(Get-DnsClientDohServerAddress -ErrorAction Stop)
        foreach ($item in $allDoh) {
            if ($item.ServerAddress) {
                $dohSettings[$item.ServerAddress] = @{
                    template = [string]$item.DohTemplate
                    allowFallback = [bool]$item.AllowFallbackToUdp
                    autoUpgrade = [bool]$item.AutoUpgrade
                }
            }
        }
    } catch {
        throw "查询全局 DoH 配置异常: $_"
    }
}

# 查询 IPv6 地址与前缀
$ipv6Addresses = @()
try {
    $ipv6Addresses = @(Get-NetIPAddress -InterfaceIndex $adapter.InterfaceIndex -AddressFamily IPv6 | Where-Object { $_.IPAddress -notlike 'fe80:*' } | Select-Object IPAddress, PrefixLength)
    if ($ipv6Addresses.Count -eq 0) {
        $ipv6Addresses = @(Get-NetIPAddress -InterfaceIndex $adapter.InterfaceIndex -AddressFamily IPv6 | Select-Object IPAddress, PrefixLength)
    }
} catch {
    if ($_.Exception.Message -notmatch 'No matching|\u627e\u4e0d\u5230|\u672a\u627e\u5230') {
        throw "查询适配器 IPv6 地址异常: $_"
    }
}

# 查询 IPv6 默认路由网关
$ipv6Gateways = @()
try {
    $ipv6Gateways = @(Get-NetRoute -InterfaceIndex $adapter.InterfaceIndex -DestinationPrefix '::/0' -AddressFamily IPv6 -ErrorAction SilentlyContinue | Select-Object -ExpandProperty NextHop)
} catch {
    if ($_.Exception.Message -notmatch 'No matching|\u627e\u4e0d\u5230|\u672a\u627e\u5230') {
        throw "查询 IPv6 默认路由网关异常: $_"
    }
}

# 查询 IPv6 DNS 服务器
$ipv6Dns = @()
try {
    $ipv6Dns = @(Get-DnsClientServerAddress -InterfaceIndex $adapter.InterfaceIndex -AddressFamily IPv6 -ErrorAction SilentlyContinue | Select-Object -ExpandProperty ServerAddresses)
} catch {
    throw "查询 IPv6 DNS 服务器异常: $_"
}

# 查询 IPv6 DHCP / SLAAC 状态
$ipv6Dhcp = $true
try {
    $ipIf6 = Get-NetIPInterface -InterfaceIndex $adapter.InterfaceIndex -AddressFamily IPv6 -ErrorAction SilentlyContinue
    if ($ipIf6) {
        $ipv6Dhcp = ($ipIf6.Dhcp -eq 'Enabled' -or $ipIf6.RouterDiscovery -eq 'Enabled')
    }
} catch {
    throw "查询 IPv6 DHCP 状态异常: $_"
}

# 判定 IPv6 DNS 是否为自动获取
$ipv6DnsDhcpEnabled = $true
$regPath6 = "HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip6\Parameters\Interfaces\$($adapter.InterfaceGuid)"
if (Test-Path $regPath6) {
    try {
        $reg6 = Get-ItemProperty -Path $regPath6 -ErrorAction Stop
        if ($reg6 -and $reg6.NameServer -and [string]$reg6.NameServer.Trim().Length -gt 0) {
            $ipv6DnsDhcpEnabled = $false
        }
    } catch {
        throw "读取网络适配器 IPv6 DNS 注册表模式异常: $_"
    }
} else {
    $ipv6DnsDhcpEnabled = $ipv6Dhcp
}

[PSCustomObject]@{
    adapterName = $adapter.Name
    interfaceIndex = $adapter.InterfaceIndex
    interfaceGuid = $adapter.InterfaceGuid
    status = $adapter.Status
    dhcpEnabled = $dhcp
    dnsDhcpEnabled = $dnsDhcpEnabled
    addresses = $ipAddresses
    gateways = $gateways
    dnsServers = $dns
    ipv6Enabled = $ipv6Enabled
    dohSettings = $dohSettings
    dohSupported = $hasDohCmdlet
    ipv6Addresses = $ipv6Addresses
    ipv6Gateways = $ipv6Gateways
    ipv6DnsServers = $ipv6Dns
    ipv6DhcpEnabled = $ipv6Dhcp
    ipv6DnsDhcpEnabled = $ipv6DnsDhcpEnabled
} | ConvertTo-Json -Compress -Depth 3
"#;

// 固定 PowerShell 脚本：应用 DoH 与 IPv6 配置，通过 stdin JSON 传递参数杜绝注入 (A-01, N-02, N-06, N-09)
const APPLY_DOH_IPV6_PS_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
[Console]::InputEncoding = [System.Text.Encoding]::UTF8
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$raw = [Console]::In.ReadToEnd()
$req = $raw | ConvertFrom-Json

$adapterName = [string]$req.adapter
$ipv6Enabled = $req.ipv6Enabled
$dohList = @($req.dohList)

# 1. 前置能力与参数校验 (N-09: 杜绝在发生修改后才因能力缺失报错)
$hasDohCmdlet = $null -ne (Get-Command Get-DnsClientDohServerAddress -ErrorAction SilentlyContinue)
foreach ($item in $dohList) {
    $mode = [string]$item.mode
    $action = [string]$item.action
    if (-not $hasDohCmdlet) {
        if ($mode -ne 'off' -and $action -ne 'remove') {
            throw "当前 Windows 系统版本缺少 DoH (DNS-over-HTTPS) 支持组件"
        }
    }
}

# 2. 设置 IPv6 绑定状态 (精确 binding 对象 InputObject，无冲突参数，值未变则跳过，N-06)
if ($null -ne $ipv6Enabled) {
    try {
        $targetBinding = Get-NetAdapterBinding -ComponentID ms_tcpip6 -ErrorAction Stop | Where-Object { $_.Name -eq $adapterName }
        if (-not $targetBinding) {
            throw "未找到匹配的网络适配器 IPv6 绑定组件: $adapterName"
        }
        $targetBinding = @($targetBinding)[0]
        if ([bool]$targetBinding.Enabled -ne [bool]$ipv6Enabled) {
            if ($ipv6Enabled) {
                Enable-NetAdapterBinding -InputObject $targetBinding -ErrorAction Stop
            } else {
                Disable-NetAdapterBinding -InputObject $targetBinding -ErrorAction Stop
            }
        }
    } catch {
        throw "修改 IPv6 状态失败: $_"
    }
}

# 3. 设置 DoH 配置 (支持 remove 动作并严格捕获删除错误，N-02, N-09)
foreach ($item in $dohList) {
    $serverIp = [string]$item.serverIp
    if (-not $serverIp) { continue }
    $mode = [string]$item.mode
    $template = [string]$item.template
    $allowFallback = [bool]$item.allowFallback
    $action = [string]$item.action

    if (-not $hasDohCmdlet) {
        # 如果系统不支持 DoH 但请求关闭或删除，优雅跳过 (N-09)
        continue
    }

    try {
        if ($action -eq 'remove') {
            # 回滚时对于新增的 DoH 条目彻底删除，严格区分不存在与删除异常 (N-02)
            $existing = $null
            try {
                $existing = Get-DnsClientDohServerAddress -ServerAddress $serverIp -ErrorAction Stop
            } catch {
                if ($_.Exception.Message -match 'No MSFT_DNSClientDohServerAddress objects found' -or $_.FullyQualifiedErrorId -match 'NoMatching') {
                    $existing = $null
                } else {
                    throw "查询待删除 DoH 服务器 ($serverIp) 失败: $_"
                }
            }
            if ($existing) {
                Remove-DnsClientDohServerAddress -ServerAddress $serverIp -ErrorAction Stop
            }
            continue
        }

        $existing = $null
        try {
            $existing = Get-DnsClientDohServerAddress -ServerAddress $serverIp -ErrorAction Stop
        } catch {
            if ($_.Exception.Message -match 'No MSFT_DNSClientDohServerAddress objects found' -or $_.FullyQualifiedErrorId -match 'NoMatching') {
                $existing = $null
            } else {
                throw "查询 DoH 服务器 ($serverIp) 失败: $_"
            }
        }

        if ($mode -eq 'off') {
            if ($existing) {
                if ($template) {
                    Set-DnsClientDohServerAddress -ServerAddress $serverIp -DohTemplate $template -AutoUpgrade $false -AllowFallbackToUdp $allowFallback -ErrorAction Stop
                } else {
                    Set-DnsClientDohServerAddress -ServerAddress $serverIp -AutoUpgrade $false -AllowFallbackToUdp $allowFallback -ErrorAction Stop
                }
            }
        } elseif ($mode -eq 'auto') {
            if ($existing) {
                Set-DnsClientDohServerAddress -ServerAddress $serverIp -AutoUpgrade $true -AllowFallbackToUdp $allowFallback -ErrorAction Stop
            } else {
                if ($template) {
                    Add-DnsClientDohServerAddress -ServerAddress $serverIp -DohTemplate $template -AllowFallbackToUdp $allowFallback -AutoUpgrade $true -ErrorAction Stop
                } else {
                    throw "服务器 $serverIp 不在内置自动 DoH 列表中，请选择手动模板模式指定 DoH 模板"
                }
            }
        } elseif ($mode -eq 'manual') {
            if (-not $template) {
                throw "服务器 $serverIp 选择手动模板模式但未填写 DoH 模板 URL"
            }
            if ($existing) {
                Set-DnsClientDohServerAddress -ServerAddress $serverIp -DohTemplate $template -AllowFallbackToUdp $allowFallback -AutoUpgrade $true -ErrorAction Stop
            } else {
                Add-DnsClientDohServerAddress -ServerAddress $serverIp -DohTemplate $template -AllowFallbackToUdp $allowFallback -AutoUpgrade $true -ErrorAction Stop
            }
        }
    } catch {
        throw "配置 DNS over HTTPS ($serverIp) 失败: $_"
    }
}

[PSCustomObject]@{ success = $true } | ConvertTo-Json -Compress
"#;

/// 获取所有网络适配器列表（使用受限固定的 PowerShell 脚本）
pub fn list_network_adapters() -> Result<Vec<AdapterInfo>, String> {
    let ps_path = get_system_binary("powershell.exe");
    let out = run_command_with_timeout(
        &ps_path,
        &[
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            LIST_ADAPTERS_PS_SCRIPT,
        ],
        None,
        Duration::from_secs(10),
    )?;

    if !out.success {
        return Err(format!("查询适配器失败: {}", out.stderr.trim()));
    }

    let txt = out.stdout.trim();
    if txt.is_empty() {
        return Ok(vec![]);
    }

    // PowerShell 在单项时可能返回单个对象，多项时返回数组
    let raw_list: Vec<serde_json::Value> = if let Ok(list) = serde_json::from_str(txt) {
        list
    } else if let Ok(single) = serde_json::from_str::<serde_json::Value>(txt) {
        vec![single]
    } else {
        return Err(format!("无法解析适配器 JSON 数据: {}", txt));
    };

    let mut result = Vec::new();
    for item in raw_list {
        let name = match item.get("Name").and_then(|v| v.as_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        let raw_status = item
            .get("Status")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string();
        let display_name = item
            .get("DisplayName")
            .and_then(|v| v.as_str())
            .unwrap_or(&name)
            .to_string();
        let interface_index = item
            .get("InterfaceIndex")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let interface_guid = item
            .get("InterfaceGuid")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let mac_address = item
            .get("MacAddress")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let friendly_status = match raw_status.as_str() {
            "Up" => "已连接",
            "Disconnected" => "网络电缆已拔出",
            "Disabled" => "已禁用",
            s => s,
        };

        let status_desc = format!("{} ({})", name, friendly_status);

        result.push(AdapterInfo {
            name,
            status: status_desc,
            raw_status,
            display_name,
            interface_index,
            interface_guid,
            mac_address,
        });
    }

    Ok(result)
}

/// 查询指定适配器的完整网络快照（单次 PowerShell 进程完成，stdin 传递参数） (A-01, A-04, A-05, R-07, F-02)
pub fn get_adapter_snapshot(adapter_target: &str) -> Result<AdapterSnapshot, String> {
    if adapter_target.trim().is_empty() {
        return Err("必须指定适配器名称或标识".to_string());
    }

    let ps_path = get_system_binary("powershell.exe");
    let input_payload = serde_json::json!({
        "target": adapter_target
    })
    .to_string();

    let out = run_command_with_timeout(
        &ps_path,
        &[
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            GET_SNAPSHOT_PS_SCRIPT,
        ],
        Some(input_payload.as_bytes()),
        Duration::from_secs(10),
    )?;

    if !out.success {
        return Err(format!("获取适配器网络配置失败: {}", out.stderr.trim()));
    }

    let txt = out.stdout.trim();
    let val: serde_json::Value = serde_json::from_str(txt)
        .map_err(|e| format!("解析网络配置失败: {}，原始输出: {}", e, txt))?;

    let adapter_name = val["adapterName"]
        .as_str()
        .ok_or_else(|| "网络配置快照缺少必需字段 adapterName".to_string())?
        .trim()
        .to_string();
    if adapter_name.is_empty() {
        return Err("网络配置快照 adapterName 为空".to_string());
    }

    let interface_index = val["interfaceIndex"]
        .as_u64()
        .ok_or_else(|| "网络配置快照缺少必需字段 interfaceIndex".to_string())?
        as u32;

    let interface_guid = val["interfaceGuid"]
        .as_str()
        .ok_or_else(|| "网络配置快照缺少必需字段 interfaceGuid".to_string())?
        .trim()
        .to_string();
    if interface_guid.is_empty() {
        return Err("网络配置快照 interfaceGuid 为空".to_string());
    }

    let status = val["status"]
        .as_str()
        .ok_or_else(|| "网络配置快照缺少必需字段 status".to_string())?
        .trim()
        .to_string();

    let dhcp_enabled = val["dhcpEnabled"]
        .as_bool()
        .ok_or_else(|| "网络配置快照缺少必需字段 dhcpEnabled".to_string())?;

    let dns_dhcp_enabled = val["dnsDhcpEnabled"]
        .as_bool()
        .ok_or_else(|| "网络配置快照缺少必需字段 dnsDhcpEnabled".to_string())?;

    // 解析地址列表并做严格校验 (F-02)
    let mut addresses = Vec::new();
    if let Some(arr) = val["addresses"].as_array() {
        for a in arr {
            let ip_str = a["IPAddress"]
                .as_str()
                .ok_or_else(|| "IPAddress 字段缺失或非字符串".to_string())?
                .trim();
            validate_ipv4(ip_str)?;
            let prefix_raw = a["PrefixLength"]
                .as_u64()
                .ok_or_else(|| "PrefixLength 字段缺失".to_string())?;
            if prefix_raw > 32 {
                return Err(format!("PrefixLength 超出有效范围 (0-32): {}", prefix_raw));
            }
            let prefix = prefix_raw as u8;
            let mask = prefix_to_subnet_mask(prefix)?;
            addresses.push(Ipv4AddressConfig {
                ip_address: ip_str.to_string(),
                prefix_length: prefix,
                mask,
            });
        }
    }

    // 解析网关列表
    let mut gateways = Vec::new();
    if let Some(arr) = val["gateways"].as_array() {
        for g in arr {
            if let Some(s) = g.as_str() {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    gateways.push(trimmed.to_string());
                }
            }
        }
    }

    // 解析 DNS 列表
    let mut dns_servers = Vec::new();
    if let Some(arr) = val["dnsServers"].as_array() {
        for d in arr {
            if let Some(s) = d.as_str() {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    dns_servers.push(trimmed.to_string());
                }
            }
        }
    }

    let ip = addresses
        .first()
        .map(|a| a.ip_address.clone())
        .unwrap_or_default();
    let mask = addresses
        .first()
        .map(|a| a.mask.clone())
        .unwrap_or_default();
    let gateway = gateways.first().cloned().unwrap_or_default();
    let dns1 = dns_servers.first().cloned().unwrap_or_default();
    let dns2 = dns_servers.get(1).cloned().unwrap_or_default();

    let ipv6_enabled = val
        .get("ipv6Enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let doh_supported = val
        .get("dohSupported")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let doh_map = val.get("dohSettings").and_then(|v| v.as_object());
    let mut doh_settings = HashMap::new();
    if let Some(map) = doh_map {
        for (server_ip, info) in map {
            let auto_upgrade = info
                .get("autoUpgrade")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let allow_fallback = info
                .get("allowFallback")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let template = info
                .get("template")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            doh_settings.insert(
                server_ip.clone(),
                DohServerSetting {
                    template,
                    allow_fallback,
                    auto_upgrade,
                },
            );
        }
    }

    let extract_doh = |dns_ip: &str| -> Option<DohConfig> {
        if dns_ip.trim().is_empty() {
            return None;
        }
        let server_info = doh_map.and_then(|m| m.get(dns_ip))?;
        let auto_upgrade = server_info
            .get("autoUpgrade")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let allow_fallback = server_info
            .get("allowFallback")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let template = server_info
            .get("template")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        if auto_upgrade {
            Some(DohConfig {
                mode: if template.is_empty() {
                    "auto".to_string()
                } else {
                    "manual".to_string()
                },
                template,
                allow_fallback,
            })
        } else {
            Some(DohConfig {
                mode: "off".to_string(),
                template,
                allow_fallback,
            })
        }
    };

    let doh1 = extract_doh(&dns1);
    let doh2 = extract_doh(&dns2);

    let mut ipv6_addresses = Vec::new();
    if let Some(arr) = val.get("ipv6Addresses").and_then(|v| v.as_array()) {
        for item in arr {
            if let (Some(ip), Some(prefix)) = (
                item.get("ipAddress").and_then(|v| v.as_str()),
                item.get("prefixLength").and_then(|v| v.as_u64()),
            ) {
                let trimmed = ip.trim();
                if !trimmed.is_empty() {
                    ipv6_addresses.push(crate::domain::Ipv6AddressConfig {
                        ip_address: trimmed.to_string(),
                        prefix_length: prefix as u8,
                    });
                }
            }
        }
    }

    let mut ipv6_gateways = Vec::new();
    if let Some(arr) = val.get("ipv6Gateways").and_then(|v| v.as_array()) {
        for g in arr {
            if let Some(s) = g.as_str() {
                let trimmed = s.trim();
                if !trimmed.is_empty() && !ipv6_gateways.contains(&trimmed.to_string()) {
                    ipv6_gateways.push(trimmed.to_string());
                }
            }
        }
    }

    let mut ipv6_dns_servers = Vec::new();
    if let Some(arr) = val.get("ipv6DnsServers").and_then(|v| v.as_array()) {
        for d in arr {
            if let Some(s) = d.as_str() {
                let trimmed = s.trim();
                if !trimmed.is_empty() && !ipv6_dns_servers.contains(&trimmed.to_string()) {
                    ipv6_dns_servers.push(trimmed.to_string());
                }
            }
        }
    }

    let ipv6_dhcp_enabled = val
        .get("ipv6DhcpEnabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let ipv6_dns_dhcp_enabled = val
        .get("ipv6DnsDhcpEnabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let primary_ipv6 = ipv6_addresses
        .iter()
        .find(|a| !a.ip_address.starts_with("fe80:") && !a.ip_address.starts_with("FE80:"))
        .or_else(|| ipv6_addresses.first());
    let ipv6_ip = primary_ipv6
        .map(|a| a.ip_address.clone())
        .unwrap_or_default();
    let ipv6_prefix = primary_ipv6.map(|a| a.prefix_length);
    let ipv6_gateway = ipv6_gateways.first().cloned().unwrap_or_default();
    let ipv6_dns1 = ipv6_dns_servers.first().cloned().unwrap_or_default();
    let ipv6_dns2 = ipv6_dns_servers.get(1).cloned().unwrap_or_default();
    let ipv6_mode = Some(if ipv6_dhcp_enabled { "dhcp" } else { "static" }.to_string());
    let ipv6_dns_mode = Some(
        if ipv6_dns_dhcp_enabled {
            "dhcp"
        } else {
            "static"
        }
        .to_string(),
    );

    Ok(AdapterSnapshot {
        adapter_name,
        interface_index,
        interface_guid,
        status,
        dhcp_enabled,
        dns_dhcp_enabled,
        addresses,
        gateways,
        dns_servers,
        ip,
        mask,
        gateway,
        dns1,
        dns2,
        doh1,
        doh2,
        ipv6_enabled,
        doh_settings,
        doh_supported,
        ipv6_mode,
        ipv6_ip,
        ipv6_prefix,
        ipv6_gateway,
        ipv6_dns_mode,
        ipv6_dns1,
        ipv6_dns2,
        ipv6_addresses,
        ipv6_gateways,
        ipv6_dns_servers,
        ipv6_dhcp_enabled,
        ipv6_dns_dhcp_enabled,
    })
}

/// 辅助函数：应用 DoH 与 IPv6 配置，通过安全 stdin JSON 传递
pub fn apply_doh_and_ipv6_internal(
    adapter_name: &str,
    ipv6_enabled: Option<bool>,
    doh_items: &[serde_json::Value],
) -> Result<(), String> {
    if ipv6_enabled.is_none() && doh_items.is_empty() {
        return Ok(());
    }

    let ps_path = get_system_binary("powershell.exe");
    let mut payload = serde_json::Map::new();
    payload.insert(
        "adapter".to_string(),
        serde_json::Value::String(adapter_name.to_string()),
    );
    if let Some(v6) = ipv6_enabled {
        payload.insert("ipv6Enabled".to_string(), serde_json::Value::Bool(v6));
    }
    payload.insert(
        "dohList".to_string(),
        serde_json::Value::Array(doh_items.to_vec()),
    );

    let input_bytes =
        serde_json::to_string(&payload).map_err(|e| format!("序列化 DoH/IPv6 参数失败: {}", e))?;

    let out = run_command_with_timeout(
        &ps_path,
        &[
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            APPLY_DOH_IPV6_PS_SCRIPT,
        ],
        Some(input_bytes.as_bytes()),
        Duration::from_secs(20),
    )?;

    if !out.success {
        return Err(format!("应用 DoH 或 IPv6 设置失败: {}", out.stderr.trim()));
    }

    Ok(())
}

/// 执行自动回滚，并在出现任何命令失败时完整汇总返回，杜绝丢弃错误 (R-03, F-01, N-02, N-04)
pub fn rollback_snapshot(target: &str, snapshot: &AdapterSnapshot) -> Result<(), String> {
    rollback_snapshot_internal(target, snapshot, &[])
}

pub fn rollback_snapshot_internal(
    target: &str,
    snapshot: &AdapterSnapshot,
    touched_dns: &[String],
) -> Result<(), String> {
    let netsh = get_system_binary("netsh.exe");
    let target_name_param = format!("name=\"{}\"", target);
    let mut errors = Vec::new();

    // 1. 恢复 IP 与网关
    if snapshot.dhcp_enabled {
        let res_ip = run_command_with_timeout(
            &netsh,
            &[
                "interface",
                "ip",
                "set",
                "address",
                &target_name_param,
                "source=dhcp",
            ],
            None,
            Duration::from_secs(15),
        );
        match res_ip {
            Ok(out) if !out.success => {
                errors.push(format!("回滚IP至DHCP失败: {}", out.stderr.trim()))
            }
            Err(e) => errors.push(format!("执行回滚IP至DHCP异常: {}", e)),
            _ => {}
        }
    } else if let Some(prev_addr) = snapshot.addresses.first() {
        let mut ip_args = vec![
            "interface",
            "ip",
            "set",
            "address",
            &target_name_param,
            "static",
            &prev_addr.ip_address,
            &prev_addr.mask,
        ];
        let prev_gw = snapshot.gateways.first().map(|s| s.as_str()).unwrap_or("");
        if !prev_gw.is_empty() {
            ip_args.push(prev_gw);
        }
        let res_ip = run_command_with_timeout(&netsh, &ip_args, None, Duration::from_secs(15));
        match res_ip {
            Ok(out) if !out.success => {
                errors.push(format!("回滚静态IP失败: {}", out.stderr.trim()))
            }
            Err(e) => errors.push(format!("执行回滚静态IP异常: {}", e)),
            _ => {}
        }

        // 恢复辅助 IP
        if snapshot.addresses.len() > 1 {
            for secondary in &snapshot.addresses[1..] {
                let add_args = [
                    "interface",
                    "ip",
                    "add",
                    "address",
                    &target_name_param,
                    &secondary.ip_address,
                    &secondary.mask,
                ];
                let res_sec =
                    run_command_with_timeout(&netsh, &add_args, None, Duration::from_secs(10));
                match res_sec {
                    Ok(out) if !out.success => errors.push(format!(
                        "恢复辅助IP ({}) 失败: {}",
                        secondary.ip_address,
                        out.stderr.trim()
                    )),
                    Err(e) => errors.push(format!(
                        "执行恢复辅助IP ({}) 异常: {}",
                        secondary.ip_address, e
                    )),
                    _ => {}
                }
            }
        }

        // 恢复辅助默认网关 (N-04)
        if snapshot.gateways.len() > 1 {
            for (idx, sec_gw) in snapshot.gateways[1..].iter().enumerate() {
                let add_gw_args = [
                    "interface",
                    "ip",
                    "add",
                    "address",
                    &target_name_param,
                    "gateway=",
                    sec_gw,
                    &format!("gwmetric={}", idx + 2),
                ];
                let res_gw =
                    run_command_with_timeout(&netsh, &add_gw_args, None, Duration::from_secs(10));
                match res_gw {
                    Ok(out) if !out.success => errors.push(format!(
                        "恢复辅助网关 ({}) 失败: {}",
                        sec_gw,
                        out.stderr.trim()
                    )),
                    Err(e) => errors.push(format!("执行恢复辅助网关 ({}) 异常: {}", sec_gw, e)),
                    _ => {}
                }
            }
        }
    } else {
        // 原配置无静态 IP 且非 DHCP，重置为 DHCP 清除先前残留 (F-01)
        let res_clean_ip = run_command_with_timeout(
            &netsh,
            &[
                "interface",
                "ip",
                "set",
                "address",
                &target_name_param,
                "source=dhcp",
            ],
            None,
            Duration::from_secs(15),
        );
        match res_clean_ip {
            Ok(out) if !out.success => {
                errors.push(format!("清除静态IP并重设为DHCP失败: {}", out.stderr.trim()))
            }
            Err(e) => errors.push(format!("执行清除静态IP并重设为DHCP异常: {}", e)),
            _ => {}
        }
    }

    // 2. 独立恢复 DNS
    if snapshot.dns_dhcp_enabled {
        let res_dns = run_command_with_timeout(
            &netsh,
            &[
                "interface",
                "ip",
                "set",
                "dns",
                &target_name_param,
                "source=dhcp",
            ],
            None,
            Duration::from_secs(15),
        );
        match res_dns {
            Ok(out) if !out.success => {
                errors.push(format!("回滚DNS至DHCP失败: {}", out.stderr.trim()))
            }
            Err(e) => errors.push(format!("执行回滚DNS至DHCP异常: {}", e)),
            _ => {}
        }
    } else if let Some(d1) = snapshot.dns_servers.first() {
        let res_d1 = run_command_with_timeout(
            &netsh,
            &[
                "interface",
                "ip",
                "set",
                "dns",
                &target_name_param,
                "static",
                d1,
            ],
            None,
            Duration::from_secs(10),
        );
        match res_d1 {
            Ok(out) if !out.success => {
                errors.push(format!("恢复首选DNS失败: {}", out.stderr.trim()))
            }
            Err(e) => errors.push(format!("执行恢复首选DNS异常: {}", e)),
            _ => {}
        }

        for (idx, d_extra) in snapshot.dns_servers.iter().skip(1).enumerate() {
            let idx_str = format!("index={}", idx + 2);
            let res_extra = run_command_with_timeout(
                &netsh,
                &[
                    "interface",
                    "ip",
                    "add",
                    "dns",
                    &target_name_param,
                    d_extra,
                    &idx_str,
                ],
                None,
                Duration::from_secs(10),
            );
            match res_extra {
                Ok(out) if !out.success => errors.push(format!(
                    "恢复辅助DNS ({}) 失败: {}",
                    d_extra,
                    out.stderr.trim()
                )),
                Err(e) => errors.push(format!("执行恢复辅助DNS ({}) 异常: {}", d_extra, e)),
                _ => {}
            }
        }
    } else {
        // 原配置无 DNS 且非 DHCP，重设为 DHCP，并严格检查命令结果，严禁吞错 (F-01)
        let res_clean_dns = run_command_with_timeout(
            &netsh,
            &[
                "interface",
                "ip",
                "set",
                "dns",
                &target_name_param,
                "source=dhcp",
            ],
            None,
            Duration::from_secs(10),
        );
        match res_clean_dns {
            Ok(out) if !out.success => {
                errors.push(format!("重设空DNS为DHCP失败: {}", out.stderr.trim()))
            }
            Err(e) => errors.push(format!("执行重设空DNS为DHCP异常: {}", e)),
            _ => {}
        }
    }

    // 3. 恢复 IPv6 与全量 DoH 设置 (还原修改项，删除新增项，N-02)
    let mut rb_doh = Vec::new();
    let mut servers_to_revert = Vec::new();
    for ip in touched_dns {
        if !servers_to_revert.contains(ip) {
            servers_to_revert.push(ip.clone());
        }
    }
    for ip in &snapshot.dns_servers {
        if !servers_to_revert.contains(ip) {
            servers_to_revert.push(ip.clone());
        }
    }

    for ip in servers_to_revert {
        if let Some(entry) = snapshot.doh_settings.get(&ip) {
            // 原先存在此条目：完整还原模板与策略 (包括 off 时的 template 与 allow_fallback，N-02)
            rb_doh.push(serde_json::json!({
                "serverIp": ip,
                "mode": if entry.auto_upgrade {
                    if entry.template.is_empty() { "auto" } else { "manual" }
                } else {
                    "off"
                },
                "template": entry.template,
                "allowFallback": entry.allow_fallback,
                "action": "set",
            }));
        } else {
            // 本次变更前不存在此 DoH 条目：回滚时彻底删除 (N-02)
            rb_doh.push(serde_json::json!({
                "serverIp": ip,
                "action": "remove",
            }));
        }
    }

    if let Err(e) = apply_doh_and_ipv6_internal(target, Some(snapshot.ipv6_enabled), &rb_doh) {
        errors.push(format!("回滚 DoH / IPv6 状态异常: {}", e));
    }

    // 恢复 IPv6 详细网络与 DNS 配置
    if snapshot.ipv6_enabled {
        if snapshot.ipv6_dhcp_enabled {
            let res_v6_dhcp = run_command_with_timeout(
                &netsh,
                &[
                    "interface",
                    "ipv6",
                    "set",
                    "interface",
                    &target_name_param,
                    "routerdiscovery=enabled",
                ],
                None,
                Duration::from_secs(15),
            );
            if let Err(e) = res_v6_dhcp {
                errors.push(format!("恢复 IPv6 自动获取异常: {}", e));
            }
        } else {
            for v6 in &snapshot.ipv6_addresses {
                let v6_prefix_str = format!("{}/{}", v6.ip_address, v6.prefix_length);
                let _ = run_command_with_timeout(
                    &netsh,
                    &[
                        "interface",
                        "ipv6",
                        "add",
                        "address",
                        &target_name_param,
                        &v6_prefix_str,
                    ],
                    None,
                    Duration::from_secs(15),
                );
            }
            if let Some(gw) = snapshot.ipv6_gateways.first() {
                let _ = run_command_with_timeout(
                    &netsh,
                    &[
                        "interface",
                        "ipv6",
                        "add",
                        "route",
                        "::/0",
                        &target_name_param,
                        gw,
                    ],
                    None,
                    Duration::from_secs(15),
                );
            }
        }

        if snapshot.ipv6_dns_dhcp_enabled {
            let _ = run_command_with_timeout(
                &netsh,
                &[
                    "interface",
                    "ipv6",
                    "set",
                    "dnsservers",
                    &target_name_param,
                    "source=dhcp",
                ],
                None,
                Duration::from_secs(15),
            );
        } else if let Some(v6_d1) = snapshot.ipv6_dns_servers.first() {
            let _ = run_command_with_timeout(
                &netsh,
                &[
                    "interface",
                    "ipv6",
                    "set",
                    "dnsservers",
                    &target_name_param,
                    "static",
                    v6_d1,
                    "validate=no",
                ],
                None,
                Duration::from_secs(15),
            );
            if let Some(v6_d2) = snapshot.ipv6_dns_servers.get(1) {
                let _ = run_command_with_timeout(
                    &netsh,
                    &[
                        "interface",
                        "ipv6",
                        "add",
                        "dnsservers",
                        &target_name_param,
                        v6_d2,
                        "index=2",
                        "validate=no",
                    ],
                    None,
                    Duration::from_secs(15),
                );
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

/// 事务式应用 IPv4 网络配置，包含事前快照、执行、读回校验、自动回滚与回滚现场真值比对 (A-01, A-03, A-06, R-01, R-02, F-01, F-05, N-02, N-03, N-07)
pub fn apply_adapter_ipv4_config_transactional(
    cfg: &Ipv4Config,
) -> Result<OperationResult, String> {
    if cfg.adapter.trim().is_empty() {
        return Err("请选择网络适配器".to_string());
    }

    // 确定协议意图模式 (N-07)
    let ip_mode = cfg
        .ip_mode
        .as_deref()
        .unwrap_or(if cfg.ip.trim().is_empty() {
            "keep"
        } else {
            "static"
        })
        .to_lowercase();
    let dns_mode = cfg
        .dns_mode
        .as_deref()
        .unwrap_or(
            if cfg.dns1.trim().is_empty() && cfg.dns2.trim().is_empty() {
                "keep"
            } else {
                "static"
            },
        )
        .to_lowercase();

    let ipv6_mode = cfg
        .ipv6_mode
        .as_deref()
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_else(|| "keep".to_string());
    let ipv6_dns_mode = cfg
        .ipv6_dns_mode
        .as_deref()
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_else(|| "keep".to_string());

    // 1. 严格网络语义校验
    let (mask_len_opt, gw_opt) = if ip_mode == "static" {
        let ip_val = validate_ipv4(&cfg.ip)?;
        let mask_len = validate_subnet_mask(&cfg.mask)?;
        let mask_val = validate_ipv4(&cfg.mask)?;
        let gw = if !cfg.gateway.trim().is_empty() {
            let gw_val = validate_ipv4(&cfg.gateway)?;
            validate_gateway_in_subnet(ip_val, mask_val, gw_val)?;
            Some(cfg.gateway.trim().to_string())
        } else {
            None
        };
        (Some(mask_len), gw)
    } else {
        (None, None)
    };

    if dns_mode == "static" {
        validate_dns_combination(&cfg.dns1, &cfg.dns2)?;
        validate_doh_config(&cfg.dns1, cfg.doh1.as_ref(), "首选 DNS")?;
        validate_doh_config(&cfg.dns2, cfg.doh2.as_ref(), "备用 DNS")?;
    }

    // IPv6 语义校验
    if ipv6_mode == "static" {
        validate_ipv6(&cfg.ipv6_ip)?;
        let prefix = cfg.ipv6_prefix.unwrap_or(64);
        validate_ipv6_prefix(prefix)?;
        if !cfg.ipv6_gateway.trim().is_empty() {
            validate_ipv6(&cfg.ipv6_gateway)?;
        }
    }
    if ipv6_dns_mode == "static" {
        validate_ipv6_dns_combination(&cfg.ipv6_dns1, &cfg.ipv6_dns2)?;
    }

    let dns1_opt = if dns_mode == "static" && !cfg.dns1.trim().is_empty() {
        Some(cfg.dns1.trim().to_string())
    } else {
        None
    };

    let dns2_opt = if dns_mode == "static" && !cfg.dns2.trim().is_empty() {
        Some(cfg.dns2.trim().to_string())
    } else {
        None
    };

    // 2. 获取变更前快照，保存现场
    let before_snapshot = get_adapter_snapshot(&cfg.adapter)?;
    if before_snapshot.status.eq_ignore_ascii_case("Disabled") {
        return Err(format!(
            "网络适配器 '{}' 当前已禁用，请先启用它",
            cfg.adapter
        ));
    }
    if !before_snapshot.dhcp_enabled && before_snapshot.addresses.is_empty() {
        return Err(
            "适配器处于异常状态 (未启用 DHCP 且无静态 IP)，无法安全执行事务修改".to_string(),
        );
    }

    // 前置能力检测：若系统不支持 DoH 但静态 DNS 请求了启用 DoH，在任何写入前停止 (N-09)
    if !before_snapshot.doh_supported && dns_mode == "static" {
        let doh1_active = cfg
            .doh1
            .as_ref()
            .map(|d| d.mode.as_str() != "off")
            .unwrap_or(false);
        let doh2_active = cfg
            .doh2
            .as_ref()
            .map(|d| d.mode.as_str() != "off")
            .unwrap_or(false);
        if doh1_active || doh2_active {
            return Err("当前 Windows 系统版本缺少 DoH (DNS-over-HTTPS) 支持组件".to_string());
        }
    }

    // 收集所有本次可能影响的 DNS 服务器，用于精确回滚 (N-02)
    let mut touched_dns = Vec::new();
    if let Some(ref d1) = dns1_opt {
        touched_dns.push(d1.clone());
    }
    if let Some(ref d2) = dns2_opt {
        if !touched_dns.contains(d2) {
            touched_dns.push(d2.clone());
        }
    }
    for d in &before_snapshot.dns_servers {
        if !touched_dns.contains(d) {
            touched_dns.push(d.clone());
        }
    }

    let netsh = get_system_binary("netsh.exe");
    let name_param = format!("name=\"{}\"", cfg.adapter);

    // 闭包：执行安全回滚并在读回后与 before_snapshot 严格比对现场真值 (F-01, N-02)
    let verify_and_build_rollback_result = |failure_reason: String| -> OperationResult {
        let rb_res = rollback_snapshot_internal(&cfg.adapter, &before_snapshot, &touched_dns);
        let current_snap_after_rb = get_adapter_snapshot(&cfg.adapter).ok();

        let (rolled_back, rb_msg) = match (rb_res, &current_snap_after_rb) {
            (Ok(()), Some(current_snap)) => {
                match verify_snapshot_restored(&before_snapshot, current_snap) {
                    Ok(()) => (
                        true,
                        format!("{failure_reason}，已自动执行安全回滚，并成功验证现场配置已完全恢复至修改前状态。"),
                    ),
                    Err(diff) => (
                        false,
                        format!(
                            "{failure_reason}，已执行回滚命令但现场核验不一致 ({diff})，原配置可能未完整恢复，请手动核对！"
                        ),
                    ),
                }
            }
            (Ok(()), None) => (
                false,
                format!("{failure_reason}，已发送回滚命令但无法读回网络快照核实，恢复状态未知，请手动核对！"),
            ),
            (Err(e), _) => (
                false,
                format!("{failure_reason}，且回滚命令执行失败: {e}。请手动检查网络配置！"),
            ),
        };

        OperationResult {
            success: false,
            message: failure_reason,
            rolled_back,
            rollback_message: Some(rb_msg),
            snapshot: current_snap_after_rb,
        }
    };

    // 3. 应用 IP 设置 (根据 ip_mode 决定执行 static / dhcp / keep，N-07)
    if ip_mode == "static" {
        let mut set_ip_args = vec![
            "interface",
            "ip",
            "set",
            "address",
            &name_param,
            "static",
            &cfg.ip,
            &cfg.mask,
        ];
        if let Some(ref gw) = gw_opt {
            set_ip_args.push(gw);
        }

        let ip_out = run_command_with_timeout(&netsh, &set_ip_args, None, Duration::from_secs(20));
        let ip_success = match &ip_out {
            Ok(out) => out.success,
            Err(_) => false,
        };

        if !ip_success {
            let err_detail = match ip_out {
                Ok(out) => format!(
                    "设置 IP 地址失败: {}\n输出: {}",
                    out.stderr.trim(),
                    out.stdout.trim()
                ),
                Err(e) => format!("执行设置 IP 地址命令异常: {}", e),
            };
            return Ok(verify_and_build_rollback_result(err_detail));
        }
    } else if ip_mode == "dhcp" && !before_snapshot.dhcp_enabled {
        // 若原先已是 DHCP，生成 no-op，绝不执行静态写入！(N-07)
        let set_dhcp_args = [
            "interface",
            "ip",
            "set",
            "address",
            &name_param,
            "source=dhcp",
        ];
        let dhcp_out =
            run_command_with_timeout(&netsh, &set_dhcp_args, None, Duration::from_secs(15));
        if !matches!(&dhcp_out, Ok(out) if out.success) {
            let err_detail = match dhcp_out {
                Ok(out) => format!("切换 IP 为 DHCP 失败: {}", out.stderr.trim()),
                Err(e) => format!("执行切换 IP 为 DHCP 命令异常: {}", e),
            };
            return Ok(verify_and_build_rollback_result(err_detail));
        }
    }

    // 4. 应用 DNS 设置 (根据 dns_mode 决定，N-07)
    let mut dns_failure = None;
    if dns_mode == "static" {
        if let Some(ref d1) = dns1_opt {
            let dns1_out = run_command_with_timeout(
                &netsh,
                &["interface", "ip", "set", "dns", &name_param, "static", d1],
                None,
                Duration::from_secs(15),
            );

            match dns1_out {
                Ok(out) if out.success => {
                    if let Some(ref d2) = dns2_opt {
                        let dns2_out = run_command_with_timeout(
                            &netsh,
                            &["interface", "ip", "add", "dns", &name_param, d2, "index=2"],
                            None,
                            Duration::from_secs(15),
                        );
                        match dns2_out {
                            Ok(out2) if out2.success => {}
                            Ok(out2) => {
                                dns_failure =
                                    Some(format!("设置辅助 DNS 失败: {}", out2.stderr.trim()))
                            }
                            Err(e) => dns_failure = Some(format!("执行设置辅助 DNS 异常: {}", e)),
                        }
                    }
                }
                Ok(out) => dns_failure = Some(format!("设置首选 DNS 失败: {}", out.stderr.trim())),
                Err(e) => dns_failure = Some(format!("执行设置首选 DNS 异常: {}", e)),
            }
        }
    } else if dns_mode == "dhcp" && !before_snapshot.dns_dhcp_enabled {
        let set_dns_dhcp = run_command_with_timeout(
            &netsh,
            &["interface", "ip", "set", "dns", &name_param, "source=dhcp"],
            None,
            Duration::from_secs(15),
        );
        if !matches!(&set_dns_dhcp, Ok(out) if out.success) {
            dns_failure = Some("切换 DNS 为 DHCP 失败".to_string());
        }
    }

    if let Some(err_msg) = dns_failure {
        return Ok(verify_and_build_rollback_result(err_msg));
    }

    // 5. 应用 DoH 与 IPv6 设置
    let mut doh_items = Vec::new();
    if dns_mode == "static" {
        if let Some(ref d1) = dns1_opt {
            if let Some(ref doh1) = cfg.doh1 {
                doh_items.push(serde_json::json!({
                    "serverIp": d1,
                    "mode": doh1.mode,
                    "template": doh1.template,
                    "allowFallback": doh1.allow_fallback,
                    "action": "set",
                }));
            }
        }
        if let Some(ref d2) = dns2_opt {
            if let Some(ref doh2) = cfg.doh2 {
                doh_items.push(serde_json::json!({
                    "serverIp": d2,
                    "mode": doh2.mode,
                    "template": doh2.template,
                    "allowFallback": doh2.allow_fallback,
                    "action": "set",
                }));
            }
        }
    }

    if cfg.ipv6_enabled.is_some() || !doh_items.is_empty() {
        if let Err(err_doh) =
            apply_doh_and_ipv6_internal(&cfg.adapter, cfg.ipv6_enabled, &doh_items)
        {
            return Ok(verify_and_build_rollback_result(err_doh));
        }
    }

    // 6. 应用 IPv6 IP 设置 (根据 ipv6_mode 决定)
    if cfg.ipv6_enabled != Some(false)
        && (before_snapshot.ipv6_enabled || cfg.ipv6_enabled == Some(true))
    {
        if ipv6_mode == "static" {
            let prefix = cfg.ipv6_prefix.unwrap_or(64);
            let ip_with_prefix = format!("{}/{}", cfg.ipv6_ip.trim(), prefix);
            for old_ip in &before_snapshot.ipv6_addresses {
                let _ = run_command_with_timeout(
                    &netsh,
                    &[
                        "interface",
                        "ipv6",
                        "delete",
                        "address",
                        &name_param,
                        &old_ip.ip_address,
                    ],
                    None,
                    Duration::from_secs(10),
                );
            }
            let add_v6_res = run_command_with_timeout(
                &netsh,
                &[
                    "interface",
                    "ipv6",
                    "add",
                    "address",
                    &name_param,
                    &ip_with_prefix,
                ],
                None,
                Duration::from_secs(15),
            );
            if !matches!(&add_v6_res, Ok(out) if out.success) {
                let err = match add_v6_res {
                    Ok(out) => format!("设置静态 IPv6 地址失败: {}", out.stderr.trim()),
                    Err(e) => format!("执行设置静态 IPv6 地址异常: {}", e),
                };
                return Ok(verify_and_build_rollback_result(err));
            }

            if !cfg.ipv6_gateway.trim().is_empty() {
                let gw = cfg.ipv6_gateway.trim();
                let _ = run_command_with_timeout(
                    &netsh,
                    &["interface", "ipv6", "delete", "route", "::/0", &name_param],
                    None,
                    Duration::from_secs(10),
                );
                let add_gw_res = run_command_with_timeout(
                    &netsh,
                    &["interface", "ipv6", "add", "route", "::/0", &name_param, gw],
                    None,
                    Duration::from_secs(15),
                );
                if !matches!(&add_gw_res, Ok(out) if out.success) {
                    let err = match add_gw_res {
                        Ok(out) => format!("设置 IPv6 默认网关失败: {}", out.stderr.trim()),
                        Err(e) => format!("执行设置 IPv6 默认网关异常: {}", e),
                    };
                    return Ok(verify_and_build_rollback_result(err));
                }
            }
        } else if ipv6_mode == "dhcp" && !before_snapshot.ipv6_dhcp_enabled {
            for old_ip in &before_snapshot.ipv6_addresses {
                let _ = run_command_with_timeout(
                    &netsh,
                    &[
                        "interface",
                        "ipv6",
                        "delete",
                        "address",
                        &name_param,
                        &old_ip.ip_address,
                    ],
                    None,
                    Duration::from_secs(10),
                );
            }
            let set_v6_dhcp = run_command_with_timeout(
                &netsh,
                &[
                    "interface",
                    "ipv6",
                    "set",
                    "interface",
                    &name_param,
                    "routerdiscovery=enabled",
                ],
                None,
                Duration::from_secs(15),
            );
            if !matches!(&set_v6_dhcp, Ok(out) if out.success) {
                let err = match set_v6_dhcp {
                    Ok(out) => format!("切换 IPv6 为自动获取失败: {}", out.stderr.trim()),
                    Err(e) => format!("执行切换 IPv6 为自动获取异常: {}", e),
                };
                return Ok(verify_and_build_rollback_result(err));
            }
        }

        // 7. 应用 IPv6 DNS 设置 (根据 ipv6_dns_mode 决定，完整支持首选与备用 IPv6 DNS)
        if ipv6_dns_mode == "static" {
            let v6_d1 = cfg.ipv6_dns1.trim();
            if !v6_d1.is_empty() {
                // 设置首选 IPv6 DNS 服务器 (Primary)
                // 采用 validate=no 防止因网络探测超时导致 netsh 挂起数十秒
                let set_v6_dns = run_command_with_timeout(
                    &netsh,
                    &[
                        "interface",
                        "ipv6",
                        "set",
                        "dnsservers",
                        &name_param,
                        "static",
                        v6_d1,
                        "validate=no",
                    ],
                    None,
                    Duration::from_secs(15),
                );
                if !matches!(&set_v6_dns, Ok(out) if out.success) {
                    let err = match set_v6_dns {
                        Ok(out) => format!("设置首选 IPv6 DNS 失败: {}", out.stderr.trim()),
                        Err(e) => format!("执行设置首选 IPv6 DNS 异常: {}", e),
                    };
                    return Ok(verify_and_build_rollback_result(err));
                }

                let v6_d2 = cfg.ipv6_dns2.trim();
                if !v6_d2.is_empty() {
                    // 追加备用 IPv6 DNS 服务器 (Secondary, 指定 index=2 建立权威主备解析顺序)
                    let add_v6_dns = run_command_with_timeout(
                        &netsh,
                        &[
                            "interface",
                            "ipv6",
                            "add",
                            "dnsservers",
                            &name_param,
                            v6_d2,
                            "index=2",
                            "validate=no",
                        ],
                        None,
                        Duration::from_secs(15),
                    );
                    if !matches!(&add_v6_dns, Ok(out) if out.success) {
                        let err = match add_v6_dns {
                            Ok(out) => format!("设置备用 IPv6 DNS 失败: {}", out.stderr.trim()),
                            Err(e) => format!("执行设置备用 IPv6 DNS 异常: {}", e),
                        };
                        return Ok(verify_and_build_rollback_result(err));
                    }
                }
            }
        } else if ipv6_dns_mode == "dhcp" && !before_snapshot.ipv6_dns_dhcp_enabled {
            let set_v6_dns_dhcp = run_command_with_timeout(
                &netsh,
                &[
                    "interface",
                    "ipv6",
                    "set",
                    "dnsservers",
                    &name_param,
                    "source=dhcp",
                ],
                None,
                Duration::from_secs(15),
            );
            if !matches!(&set_v6_dns_dhcp, Ok(out) if out.success) {
                let err = match set_v6_dns_dhcp {
                    Ok(out) => format!("切换 IPv6 DNS 为自动获取失败: {}", out.stderr.trim()),
                    Err(e) => format!("执行切换 IPv6 DNS 为自动获取异常: {}", e),
                };
                return Ok(verify_and_build_rollback_result(err));
            }
        }
    }

    // 6. 读回并校验生效状态 (收敛重试，最多 3 秒，严格覆盖各模式期望) (R-02, F-05, N-03, N-07)
    let start_wait = Instant::now();
    let max_wait = Duration::from_secs(3);
    let mut final_snapshot = None;
    let mut verify_err = None;

    while start_wait.elapsed() < max_wait {
        std::thread::sleep(Duration::from_millis(300));
        match get_adapter_snapshot(&cfg.adapter) {
            Ok(s) => {
                let mut reasons = Vec::new();

                // 1. 网卡身份标识比对 (F-05)
                if s.interface_guid != before_snapshot.interface_guid {
                    reasons.push(format!(
                        "网卡身份变更 (原GUID: {}, 当前: {})",
                        before_snapshot.interface_guid, s.interface_guid
                    ));
                }

                // 2. IP 地址及 DHCP 模式核验 (N-07)
                if ip_mode == "static" {
                    if s.dhcp_enabled {
                        reasons.push("IP DHCP未关闭".to_string());
                    }
                    if let Some(mask_len) = mask_len_opt {
                        let ip_ok = s
                            .addresses
                            .iter()
                            .any(|a| a.ip_address == cfg.ip && a.prefix_length == mask_len);
                        if !ip_ok {
                            reasons.push("IP地址或掩码未匹配".to_string());
                        }
                    }
                    if let Some(ref gw) = gw_opt {
                        if !s.gateways.iter().any(|g| g == gw) {
                            reasons.push("默认网关未匹配".to_string());
                        }
                    }
                } else if ip_mode == "dhcp" && !s.dhcp_enabled {
                    reasons.push("IP DHCP未能开启".to_string());
                }

                // 3. DNS 顺序与模式严格比对 (F-05, N-07)
                if dns_mode == "static" {
                    if dns1_opt.is_some() || dns2_opt.is_some() {
                        if s.dns_dhcp_enabled {
                            reasons.push("DNS DHCP未关闭".to_string());
                        }
                        if let Some(ref d1) = dns1_opt {
                            if s.dns_servers.first() != Some(d1) {
                                reasons.push(format!(
                                    "首选DNS顺序或值不匹配 (期望: {}, 实际: {:?})",
                                    d1,
                                    s.dns_servers.first()
                                ));
                            }
                        }
                        if let Some(ref d2) = dns2_opt {
                            if s.dns_servers.get(1) != Some(d2) {
                                reasons.push(format!(
                                    "辅助DNS顺序或值不匹配 (期望: {}, 实际: {:?})",
                                    d2,
                                    s.dns_servers.get(1)
                                ));
                            }
                        }
                    }
                } else if dns_mode == "dhcp" && !s.dns_dhcp_enabled {
                    reasons.push("DNS DHCP未能开启".to_string());
                }

                // 4. IPv6 状态核验
                if let Some(expected_ipv6) = cfg.ipv6_enabled {
                    if s.ipv6_enabled != expected_ipv6 {
                        reasons.push(format!(
                            "IPv6状态未匹配 (期望: {}, 实际: {})",
                            expected_ipv6, s.ipv6_enabled
                        ));
                    }
                }

                // 5. DoH 状态严格核验 (关闭状态、模板URL、明文回退，N-03)
                if dns_mode == "static" {
                    let verify_doh_readback = |exp_opt: Option<&DohConfig>,
                                               act_opt: Option<&DohConfig>,
                                               label: &str|
                     -> Option<String> {
                        let exp = exp_opt?;
                        let act_mode = act_opt.map(|d| d.mode.as_str()).unwrap_or("off");
                        if exp.mode == "off" {
                            if act_mode != "off" {
                                return Some(format!(
                                    "{} DoH未成功关闭 (当前仍为: {})",
                                    label, act_mode
                                ));
                            }
                        } else {
                            if act_mode == "off" {
                                return Some(format!("{} DoH未成功开启", label));
                            }
                            if exp.mode == "manual" {
                                let act_tmpl = act_opt.map(|d| d.template.as_str()).unwrap_or("");
                                if act_tmpl != exp.template.trim() {
                                    return Some(format!(
                                        "{} DoH模板未匹配 (期望: {}, 实际: {})",
                                        label, exp.template, act_tmpl
                                    ));
                                }
                            }
                            let act_fb = act_opt.map(|d| d.allow_fallback).unwrap_or(true);
                            if act_fb != exp.allow_fallback {
                                return Some(format!(
                                    "{} DoH明文回退未匹配 (期望: {}, 实际: {})",
                                    label, exp.allow_fallback, act_fb
                                ));
                            }
                        }
                        None
                    };

                    if let Some(err) =
                        verify_doh_readback(cfg.doh1.as_ref(), s.doh1.as_ref(), "首选DNS")
                    {
                        reasons.push(err);
                    }
                    if let Some(err) =
                        verify_doh_readback(cfg.doh2.as_ref(), s.doh2.as_ref(), "备用DNS")
                    {
                        reasons.push(err);
                    }
                }

                // 6. IPv6 读回校验
                if s.ipv6_enabled {
                    if ipv6_mode == "static" {
                        let exp_ip = cfg.ipv6_ip.trim();
                        let exp_prefix = cfg.ipv6_prefix.unwrap_or(64);
                        if !s.ipv6_addresses.iter().any(|a| {
                            a.ip_address.eq_ignore_ascii_case(exp_ip)
                                && a.prefix_length == exp_prefix
                        }) {
                            reasons.push(format!(
                                "静态 IPv6 地址未生效 (期望: {}/{})",
                                exp_ip, exp_prefix
                            ));
                        }
                    } else if ipv6_mode == "dhcp" && !s.ipv6_dhcp_enabled {
                        reasons.push("IPv6 自动获取未开启".to_string());
                    }

                    if ipv6_dns_mode == "static" {
                        if !cfg.ipv6_dns1.trim().is_empty() {
                            let exp_d1 = cfg.ipv6_dns1.trim();
                            if s.ipv6_dns_servers.first().map(|s| s.as_str()) != Some(exp_d1) {
                                reasons.push(format!(
                                    "首选 IPv6 DNS 未生效 (期望: {}, 实际: {:?})",
                                    exp_d1,
                                    s.ipv6_dns_servers.first()
                                ));
                            }
                        }
                    } else if ipv6_dns_mode == "dhcp" && !s.ipv6_dns_dhcp_enabled {
                        reasons.push("IPv6 DNS 自动获取未开启".to_string());
                    }
                }

                if reasons.is_empty() {
                    final_snapshot = Some(s);
                    verify_err = None;
                    break;
                } else {
                    verify_err = Some(format!("读回校验未通过 ({})", reasons.join(", ")));
                    final_snapshot = Some(s);
                }
            }
            Err(e) => {
                verify_err = Some(format!("读回配置快照失败: {}", e));
            }
        }
    }

    if let Some(err_msg) = verify_err {
        return Ok(verify_and_build_rollback_result(err_msg));
    }

    Ok(OperationResult {
        success: true,
        message: "网络配置已成功应用并通过读回校验".to_string(),
        rolled_back: false,
        rollback_message: None,
        snapshot: final_snapshot,
    })
}
