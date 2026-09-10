pub mod domain;
pub mod platform;

use domain::{AdapterInfo, AdapterSnapshot, Ipv4Config, OperationResult};
use tauri::async_runtime::Mutex;
use tauri::State;

/// 全局网络配置写操作锁，确保当前进程内写操作串行化执行 (A-03, A-04)
pub struct NetworkLock(pub Mutex<()>);

#[cfg(windows)]
mod sys_mutex {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::null_mut;

    type RawHandle = *mut std::ffi::c_void;
    const WAIT_OBJECT_0: u32 = 0x00000000;
    const WAIT_ABANDONED: u32 = 0x00000080;
    const WAIT_TIMEOUT: u32 = 0x00000102;
    const ERROR_ACCESS_DENIED: u32 = 5;

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateMutexW(
            lpMutexAttributes: *mut std::ffi::c_void,
            bInitialOwner: i32,
            lpName: *const u16,
        ) -> RawHandle;
        fn WaitForSingleObject(hHandle: RawHandle, dwMilliseconds: u32) -> u32;
        fn ReleaseMutex(hMutex: RawHandle) -> i32;
        fn CloseHandle(hObject: RawHandle) -> i32;
        fn GetLastError() -> u32;
    }

    #[derive(Debug, PartialEq, Eq)]
    pub enum MutexError {
        AccessDenied,
        Timeout,
        SystemError(String),
    }

    /// 跨进程具名互斥体保护，防止多个应用实例交错修改系统网络 (R-08, F-07, N-01)
    pub struct CrossProcessLock {
        handle: RawHandle,
    }

    impl CrossProcessLock {
        pub fn acquire_raw(name: &str, timeout_ms: u32) -> Result<Self, MutexError> {
            let wide: Vec<u16> = OsStr::new(name)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let handle = unsafe { CreateMutexW(null_mut(), 0, wide.as_ptr()) };
            if handle.is_null() {
                let err = unsafe { GetLastError() };
                if err == ERROR_ACCESS_DENIED {
                    return Err(MutexError::AccessDenied);
                }
                return Err(MutexError::SystemError(format!(
                    "无法创建或打开系统命名互斥体 ({})，错误码: {}",
                    name, err
                )));
            }
            let res = unsafe { WaitForSingleObject(handle, timeout_ms) };
            if res == WAIT_OBJECT_0 || res == WAIT_ABANDONED {
                Ok(CrossProcessLock { handle })
            } else if res == WAIT_TIMEOUT {
                unsafe { CloseHandle(handle) };
                Err(MutexError::Timeout)
            } else {
                unsafe { CloseHandle(handle) };
                let err = unsafe { GetLastError() };
                Err(MutexError::SystemError(format!(
                    "等待系统配置互斥锁失败，结果码: {:#x}, 错误码: {}",
                    res, err
                )))
            }
        }

        pub fn acquire(name: &str, timeout_ms: u32) -> Result<Self, String> {
            match Self::acquire_raw(name, timeout_ms) {
                Ok(lock) => Ok(lock),
                Err(MutexError::Timeout) => Err(
                    "系统检测到另一个网络配置进程正在执行，请稍候再试 (跨进程互斥保护)".to_string(),
                ),
                Err(MutexError::AccessDenied) => {
                    Err("权限不足，无法访问系统全局互斥锁，请使用管理员权限运行本程序".to_string())
                }
                Err(MutexError::SystemError(msg)) => Err(msg),
            }
        }

        /// 获取跨会话 Global 互斥体，严格保证系统级单实例修改网络 (F-07, N-01)
        /// 严禁将争用超时 (WAIT_TIMEOUT) 或权限不足回退至 Local 锁，杜绝跨权限等级进程互斥失效！
        pub fn acquire_global_or_local(base_name: &str, timeout_ms: u32) -> Result<Self, String> {
            let global_name = format!("Global\\{}", base_name);
            Self::acquire(&global_name, timeout_ms)
        }
    }

    impl Drop for CrossProcessLock {
        fn drop(&mut self) {
            if !self.handle.is_null() {
                unsafe {
                    ReleaseMutex(self.handle);
                    CloseHandle(self.handle);
                }
            }
        }
    }

    unsafe impl Send for CrossProcessLock {}
}

pub mod commands {
    use super::*;

    #[tauri::command]
    pub fn greet(name: &str) -> String {
        format!("Hello, {}! You've been greeted from Rust!", name)
    }

    /// 异步获取所有网络适配器列表，避免阻塞 UI 线程 (A-04)
    #[tauri::command]
    pub async fn get_network_adapters() -> Result<Vec<AdapterInfo>, String> {
        tauri::async_runtime::spawn_blocking(platform::list_network_adapters)
            .await
            .map_err(|e| format!("调度查询适配器任务失败: {}", e))?
    }

    /// 异步单次提取适配器完整网络快照 (A-04, A-05)
    #[tauri::command]
    pub async fn get_current_config(adapter_name: String) -> Result<AdapterSnapshot, String> {
        tauri::async_runtime::spawn_blocking(move || platform::get_adapter_snapshot(&adapter_name))
            .await
            .map_err(|e| format!("调度获取当前配置任务失败: {}", e))?
    }

    /// 事务式应用 IPv4 配置，包含事前快照、校验、跨进程互斥锁与自动回滚 (A-01, A-03, A-06, R-08, F-07)
    #[tauri::command]
    pub async fn apply_adapter_ipv4_config(
        cfg: Ipv4Config,
        lock: State<'_, NetworkLock>,
    ) -> Result<OperationResult, String> {
        let _guard = lock.0.lock().await;
        tauri::async_runtime::spawn_blocking(move || {
            #[cfg(windows)]
            let _sys_guard = sys_mutex::CrossProcessLock::acquire_global_or_local(
                "WindowsNetworkConfigTool_WriteLock",
                5000,
            )?;

            platform::apply_adapter_ipv4_config_transactional(&cfg)
        })
        .await
        .map_err(|e| format!("调度应用配置任务失败: {}", e))?
    }
}

/// 启动 Tauri 应用程序
pub fn run() {
    tauri::Builder::default()
        .manage(NetworkLock(Mutex::new(())))
        .invoke_handler(tauri::generate_handler![
            commands::greet,
            commands::get_network_adapters,
            commands::apply_adapter_ipv4_config,
            commands::get_current_config
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    #[test]
    fn test_cross_process_lock_timeout_prevents_local_bypass() {
        use super::sys_mutex::CrossProcessLock;
        let test_lock_name = format!("Test_Audit_Lock_{}", std::process::id());
        let global_name = format!("Global\\{}", test_lock_name);

        let lock_a = match CrossProcessLock::acquire(&global_name, 100) {
            Ok(l) => l,
            Err(e) if e.contains("权限不足") || e.contains("无法创建") => {
                return;
            }
            Err(e) => panic!("意外失败: {}", e),
        };

        // Win32 互斥体对同一线程是可重入的，因此必须在独立工作线程中验证争用与超时
        let lock_name_clone = test_lock_name.clone();
        let res_b = std::thread::spawn(move || {
            CrossProcessLock::acquire_global_or_local(&lock_name_clone, 100).map(|_| ())
        })
        .join()
        .expect("工作线程异常");

        assert!(
            res_b.is_err(),
            "当 Global 锁被持有时，跨线程/进程 acquire_global_or_local 绝不能回退至 Local 获得成功！"
        );
        let err_msg = res_b.err().unwrap();
        assert!(
            err_msg.contains("跨进程互斥保护") || err_msg.contains("正在执行"),
            "错误消息必须表明超时争用繁忙: {}",
            err_msg
        );

        drop(lock_a);
    }

    #[cfg(windows)]
    #[test]
    fn test_run_command_silent_background_execution() {
        use std::time::Duration;
        let ps = super::platform::get_system_binary("powershell.exe");
        let out = super::platform::run_command_with_timeout(
            &ps,
            &[
                "-NoProfile",
                "-NonInteractive",
                "-WindowStyle",
                "Hidden",
                "-Command",
                "Write-Output 'silent_background_ok'",
            ],
            None,
            Duration::from_secs(10),
        )
        .expect("后台无窗口命令必须成功执行");
        assert!(out.success);
        assert_eq!(out.stdout.trim(), "silent_background_ok");
    }
}
