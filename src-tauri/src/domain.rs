use std::collections::HashMap;
use std::net::{Ipv4Addr, Ipv6Addr};

/// 网络适配器概要信息 (用于前端网卡列表渲染与状态概览)
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AdapterInfo {
    /// 适配器接口名称 (例如: "以太网", "WLAN")
    pub name: String,
    /// 用户友好的状态展示文本 (例如: "已连接", "未连接")
    pub status: String,
    /// 系统底层原始状态值 (如 "Up", "Disconnected")
    pub raw_status: String,
    /// 硬件网卡驱动描述名称 (如 "Realtek Gaming 2.5GbE Family Controller")
    pub display_name: String,
    /// 网卡接口数字索引 (ifIndex)
    pub interface_index: u32,
    /// 网卡唯一全局标识符 GUID
    pub interface_guid: String,
    /// 物理 MAC 地址 (如 "00-1A-2B-3C-4D-5E")
    pub mac_address: Option<String>,
}

/// 单个 IPv4 地址及掩码信息
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Ipv4AddressConfig {
    /// IPv4 地址点分十进制字符串
    pub ip_address: String,
    /// 网络前缀长度 (0..=32，如 24 对应 255.255.255.0)
    pub prefix_length: u8,
    /// 点分十进制子网掩码字符串
    pub mask: String,
}

/// 单个 IPv6 地址及前缀长度信息
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Ipv6AddressConfig {
    /// IPv6 地址字符串
    pub ip_address: String,
    /// 网络前缀长度 (1..=128，通常为 64)
    pub prefix_length: u8,
    /// Windows 地址来源；旧快照缺失时保留未知，不能推断为手动地址。
    #[serde(default)]
    pub prefix_origin: String,
    #[serde(default)]
    pub suffix_origin: String,
}

impl Ipv6AddressConfig {
    pub fn is_manual(&self) -> bool {
        self.prefix_origin.eq_ignore_ascii_case("Manual")
            && self.suffix_origin.eq_ignore_ascii_case("Manual")
    }

    pub fn is_automatic(&self) -> bool {
        (self.prefix_origin.eq_ignore_ascii_case("Dhcp")
            && self.suffix_origin.eq_ignore_ascii_case("Dhcp"))
            || (self
                .prefix_origin
                .eq_ignore_ascii_case("RouterAdvertisement")
                && ["Link", "Random"]
                    .iter()
                    .any(|s| self.suffix_origin.eq_ignore_ascii_case(s)))
            || (self.prefix_origin.eq_ignore_ascii_case("WellKnown")
                && self.suffix_origin.eq_ignore_ascii_case("Link"))
    }
}

/// 对照修改前的完整地址状态核验实际触及的项；自动地址可重新分配，手动写入必须撤销。
pub fn verify_ipv6_address_rollback(
    before: &AdapterSnapshot,
    current: &AdapterSnapshot,
    touched: &[String],
) -> Result<(), String> {
    for ip in touched {
        let original = before
            .ipv6_addresses
            .iter()
            .find(|a| ipv6_addr_eq(&a.ip_address, ip));
        let actual: Vec<_> = current
            .ipv6_addresses
            .iter()
            .filter(|a| ipv6_addr_eq(&a.ip_address, ip))
            .collect();
        match original {
            Some(old) if old.is_manual() => {
                if actual.len() != 1
                    || actual[0].prefix_length != old.prefix_length
                    || !actual[0].is_manual()
                {
                    return Err(format!(
                        "原手动 IPv6 地址 {}/{} 的前缀或来源未恢复",
                        ip, old.prefix_length
                    ));
                }
            }
            Some(old) if !old.is_automatic() => {
                return Err(format!("IPv6 地址 {} 的原始来源未知，无法确认恢复", ip))
            }
            _ => {
                if actual.iter().any(|a| !a.is_automatic()) {
                    return Err(format!("本次手动 IPv6 地址 {} 仍残留或来源无法核实", ip));
                }
            }
        }
    }
    Ok(())
}

/// DNS over HTTPS (DoH) 加密解析配置项
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DohConfig {
    /// 加密模式: "off" (关闭) | "auto" (自动升级) | "manual" (手动指定 HTTPS 模板)
    pub mode: String,
    /// DoH 模板 URL (如 https://doh.pub/dns-query, 仅 manual 模式必需)
    #[serde(default)]
    pub template: String,
    /// 解析失败时是否允许回退到未加密常规 DNS 请求 (容灾保证可用性)
    #[serde(default)]
    pub allow_fallback: bool,
}

impl Default for DohConfig {
    fn default() -> Self {
        Self {
            mode: "off".to_string(),
            template: String::new(),
            allow_fallback: true,
        }
    }
}

/// Windows 11 系统级全局 DoH 服务器条目配置 (N-02)
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DohServerSetting {
    /// 该 DNS 服务器绑定的 HTTPS 模板 URL
    #[serde(default)]
    pub template: String,
    /// 是否允许降级为未加密请求
    #[serde(default)]
    pub allow_fallback: bool,
    /// 是否允许系统自动升级为 DoH
    #[serde(default)]
    pub auto_upgrade: bool,
}

/// 单网卡 DoH 原始属性。None 与空字符串/零值不同，回滚必须保留属性缺失状态。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AdapterDohSetting {
    pub flags: Option<u64>,
    pub template: Option<String>,
}

/// 适配器全息运行时快照（包含当前生效的 IPv4/IPv6/DNS/DoH 状态，兼备单项便捷字段）
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AdapterSnapshot {
    /// 目标适配器别名
    pub adapter_name: String,
    /// 接口数字索引
    pub interface_index: u32,
    /// 接口唯一 GUID
    pub interface_guid: String,
    /// 连接状态
    pub status: String,
    /// IPv4 是否启用了 DHCP 自动获取
    pub dhcp_enabled: bool,
    /// IPv4 DNS 是否由 DHCP 自动下发
    #[serde(default)]
    pub dns_dhcp_enabled: bool,
    /// 适配器绑定的全部 IPv4 地址集合
    pub addresses: Vec<Ipv4AddressConfig>,
    /// 适配器当前生效的 IPv4 默认网关集合
    pub gateways: Vec<String>,
    /// 适配器当前生效的 IPv4 DNS 服务器集合 (首项为首选，次项为备用)
    pub dns_servers: Vec<String>,
    // 兼容一维便捷字段
    pub ip: String,
    pub mask: String,
    pub gateway: String,
    pub dns1: String,
    pub dns2: String,
    // DoH 与 IPv6 扩展字段
    /// 首选 DNS 绑定的 DoH 加密状态
    #[serde(default)]
    pub doh1: Option<DohConfig>,
    /// 备用 DNS 绑定的 DoH 加密状态
    #[serde(default)]
    pub doh2: Option<DohConfig>,
    /// IPv6 协议组件 (ms_tcpip6) 是否处于启用状态
    #[serde(default = "default_true")]
    pub ipv6_enabled: bool,
    /// Windows 注册表/系统级已注册的 DoH 服务器表 (N-02)
    #[serde(default)]
    pub doh_settings: HashMap<String, DohServerSetting>,
    #[serde(default)]
    pub has_interface_doh: bool,
    #[serde(default)]
    pub adapter_doh_settings: HashMap<String, AdapterDohSetting>,
    /// 系统内核是否支持 DoH
    #[serde(default = "default_true")]
    pub doh_supported: bool,
    // IPv6 详细扩展字段
    /// IPv6 地址分配模式 ("dhcp" | "static")
    #[serde(default)]
    pub ipv6_mode: Option<String>,
    /// 静态 IPv6 地址
    #[serde(default)]
    pub ipv6_ip: String,
    /// 静态 IPv6 前缀长度
    #[serde(default)]
    pub ipv6_prefix: Option<u8>,
    /// 静态 IPv6 默认网关
    #[serde(default)]
    pub ipv6_gateway: String,
    /// IPv6 DNS 分配模式 ("dhcp" | "static")
    #[serde(default)]
    pub ipv6_dns_mode: Option<String>,
    /// 首选 IPv6 DNS (Primary)
    #[serde(default)]
    pub ipv6_dns1: String,
    /// 备用 IPv6 DNS (Secondary)
    #[serde(default)]
    pub ipv6_dns2: String,
    /// 绑定的全部 IPv6 地址集合
    #[serde(default)]
    pub ipv6_addresses: Vec<Ipv6AddressConfig>,
    /// 生效的全部 IPv6 默认网关集合
    #[serde(default)]
    pub ipv6_gateways: Vec<String>,
    /// 生效的全部 IPv6 DNS 服务器集合
    #[serde(default)]
    pub ipv6_dns_servers: Vec<String>,
    /// IPv6 地址是否由 DHCPv6 / SLAAC 自动获取
    #[serde(default = "default_true")]
    pub ipv6_dhcp_enabled: bool,
    /// IPv6 DNS 是否由 DHCPv6 自动下发
    #[serde(default = "default_true")]
    pub ipv6_dns_dhcp_enabled: bool,
}

fn default_true() -> bool {
    true
}

/// 前端提交的网络配置变更意图 (支持显式协议意图: "dhcp" | "static" | "keep") (N-07)
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Ipv4Config {
    /// 目标适配器别名
    pub adapter: String,
    /// IPv4 地址分配意图 ("dhcp" | "static" | "keep")
    #[serde(default)]
    pub ip_mode: Option<String>,
    /// 静态 IPv4 地址 (静态模式必填)
    #[serde(default)]
    pub ip: String,
    /// 静态 IPv4 子网掩码 (静态模式必填)
    #[serde(default)]
    pub mask: String,
    /// 静态 IPv4 默认网关 (可选)
    #[serde(default)]
    pub gateway: String,
    /// IPv4 DNS 分配意图 ("dhcp" | "static" | "keep")
    #[serde(default)]
    pub dns_mode: Option<String>,
    /// 首选 IPv4 DNS
    #[serde(default)]
    pub dns1: String,
    /// 备用 IPv4 DNS
    #[serde(default)]
    pub dns2: String,
    /// 首选 DoH 配置
    #[serde(default)]
    pub doh1: Option<DohConfig>,
    /// 备用 DoH 配置
    #[serde(default)]
    pub doh2: Option<DohConfig>,
    /// 是否开启此适配器的 IPv6 协议组件 (None 表示不变更)
    #[serde(default)]
    pub ipv6_enabled: Option<bool>,
    /// IPv6 地址分配意图 ("dhcp" | "static" | "keep")
    #[serde(default)]
    pub ipv6_mode: Option<String>,
    /// 静态 IPv6 地址
    #[serde(default)]
    pub ipv6_ip: String,
    /// 静态 IPv6 前缀长度 (1..=128)
    #[serde(default)]
    pub ipv6_prefix: Option<u8>,
    /// 静态 IPv6 默认网关
    #[serde(default)]
    pub ipv6_gateway: String,
    /// IPv6 DNS 分配意图 ("dhcp" | "static" | "keep")
    #[serde(default)]
    pub ipv6_dns_mode: Option<String>,
    /// 首选 IPv6 DNS 服务器 (Primary)
    #[serde(default)]
    pub ipv6_dns1: String,
    /// 备用 IPv6 DNS 服务器 (Secondary)
    #[serde(default)]
    pub ipv6_dns2: String,
}

/// 校验 DoH 配置
pub fn validate_doh_config(
    dns_ip: &str,
    doh: Option<&DohConfig>,
    label: &str,
) -> Result<(), String> {
    let Some(cfg) = doh else {
        return Ok(());
    };

    let mode = cfg.mode.trim().to_lowercase();
    if mode == "off" || mode.is_empty() {
        return Ok(());
    }

    if dns_ip.trim().is_empty() {
        return Err(format!(
            "启用了{}的 DNS over HTTPS，但未配置该 DNS 服务器的 IP 地址",
            label
        ));
    }

    if mode == "manual" {
        let t = cfg.template.trim();
        if t.is_empty() {
            return Err(format!(
                "{}已选择手动模板模式，必须填写 DoH 模板 URL",
                label
            ));
        }
        if !t.starts_with("https://") {
            return Err(format!(
                "{}的 DoH 模板必须是以 https:// 开头的合法 URL",
                label
            ));
        }
        if t.len() < 10 || !t[8..].contains('/') {
            return Err(format!(
                "{}的 DoH 模板格式不完整 (如 https://doh.pub/dns-query)",
                label
            ));
        }
    } else if mode != "auto" {
        return Err(format!(
            "{}的 DoH 模式 '{}' 不受支持 (仅支持 off / auto / manual)",
            label, cfg.mode
        ));
    }

    Ok(())
}

/// 校验 DNS 配置组合（不允许未配置首选 DNS 而单独配置辅助 DNS）
pub fn validate_dns_combination(dns1: &str, dns2: &str) -> Result<(), String> {
    let d1 = dns1.trim();
    let d2 = dns2.trim();
    if d1.is_empty() && !d2.is_empty() {
        return Err("若配置辅助 DNS，必须先配置首选 DNS (DNS1)".to_string());
    }
    if !d1.is_empty() {
        validate_ipv4(d1)?;
    }
    if !d2.is_empty() {
        validate_ipv4(d2)?;
    }
    Ok(())
}

/// 配置应用操作结果
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OperationResult {
    pub success: bool,
    pub message: String,
    pub rolled_back: bool,
    pub rollback_message: Option<String>,
    pub snapshot: Option<AdapterSnapshot>,
}

/// 严格校验 IPv6 地址合法性
pub fn validate_ipv6(ip: &str) -> Result<Ipv6Addr, String> {
    let trimmed = ip.trim();
    if trimmed.is_empty() {
        return Err("IPv6 地址不能为空".to_string());
    }

    // 处理可能包含的 scope/zone ID (例如 fe80::1%12)
    let clean_ip = if let Some((addr_part, _)) = trimmed.split_once('%') {
        addr_part
    } else {
        trimmed
    };

    clean_ip
        .parse::<Ipv6Addr>()
        .map_err(|e| format!("无效的 IPv6 地址 '{}': {}", trimmed, e))
}

/// 校验 IPv6 前缀长度 (1..=128，通常为 64)
pub fn validate_ipv6_prefix(prefix: u8) -> Result<(), String> {
    if prefix == 0 || prefix > 128 {
        return Err(format!(
            "IPv6 前缀长度必须在 1 到 128 之间 (常用值为 64)，当前值: {}",
            prefix
        ));
    }
    Ok(())
}

/// 解析 IPv6 地址字符串，分离基础 IP 地址与可选 Scope/Zone ID
pub fn parse_ipv6_with_scope(s: &str) -> Option<(Ipv6Addr, Option<&str>)> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (ip_part, scope_part) = match trimmed.split_once('%') {
        Some((ip, scope)) => (ip.trim(), Some(scope.trim())),
        None => (trimmed, None),
    };
    let ip = ip_part.parse::<Ipv6Addr>().ok()?;
    Some((ip, scope_part.filter(|s| !s.is_empty())))
}

/// 规范化比对两个 IPv6 地址是否在网络语义上完全等价 (V6-07)
///
/// 支持解析并比对包含 Scope ID (如 fe80::1%12)、零压缩 (2001:db8::20 与 2001:0db8:0:0:0:0:0:20) 以及大小写无关性。
pub fn ipv6_addr_eq(a: &str, b: &str) -> bool {
    match (parse_ipv6_with_scope(a), parse_ipv6_with_scope(b)) {
        (Some((ip_a, scope_a)), Some((ip_b, scope_b))) => {
            ip_a == ip_b
                && scope_a.map(|s| s.to_ascii_lowercase())
                    == scope_b.map(|s| s.to_ascii_lowercase())
        }
        _ => a.trim().eq_ignore_ascii_case(b.trim()),
    }
}

/// 手动 IPv6 DNS 必须填写首选 DNS；保留现有配置应使用 keep 模式。
pub fn validate_ipv6_dns_combination(dns1: &str, dns2: &str) -> Result<(), String> {
    let d1 = dns1.trim();
    let d2 = dns2.trim();
    if d1.is_empty() {
        return Err("手动 IPv6 DNS 必须填写首选 DNS (DNS1)，如不修改请选择保持现状".to_string());
    }
    if !d1.is_empty() {
        validate_ipv6(d1)?;
    }
    if !d2.is_empty() {
        validate_ipv6(d2)?;
    }
    Ok(())
}

/// 严格校验 IPv4 地址（必须有 4 段，0-255，且不能有前导零）
pub fn validate_ipv4(ip: &str) -> Result<Ipv4Addr, String> {
    let trimmed = ip.trim();
    if trimmed.is_empty() {
        return Err("IP地址不能为空".to_string());
    }

    let parts: Vec<&str> = trimmed.split('.').collect();
    if parts.len() != 4 {
        return Err("IP地址格式不正确，必须为四段点分十进制数 (如 192.168.1.100)".to_string());
    }

    for part in &parts {
        if part.is_empty() {
            return Err("IP地址段不能为空".to_string());
        }
        if part.len() > 1 && part.starts_with('0') {
            return Err(format!("IP地址段不能包含前导零: '{}'", part));
        }
        match part.parse::<u8>() {
            Ok(_) => {}
            Err(_) => return Err(format!("IP地址段必须为 0-255 之间的整数: '{}'", part)),
        }
    }

    trimmed
        .parse::<Ipv4Addr>()
        .map_err(|e| format!("无效的 IPv4 地址: {}", e))
}

/// 严格校验子网掩码（必须是合法的连续二进制 1，前缀在 0..=32 之间）
pub fn validate_subnet_mask(mask: &str) -> Result<u8, String> {
    let mask_ip = validate_ipv4(mask)?;
    let mask_u32 = u32::from(mask_ip);

    let leading_ones = mask_u32.leading_ones();
    let trailing_zeros = mask_u32.trailing_zeros();

    if leading_ones + trailing_zeros != 32 {
        return Err(format!(
            "子网掩码 '{}' 非连续，必须为最高位连续的 1 (如 255.255.255.0)",
            mask
        ));
    }

    Ok(leading_ones as u8)
}

/// 将前缀长度 (0..=32) 转换为标准点分十进制掩码字符串
pub fn prefix_to_subnet_mask(prefix_length: u8) -> Result<String, String> {
    if prefix_length > 32 {
        return Err(format!("前缀长度超出范围 (0-32): {}", prefix_length));
    }

    let mask_bits: u32 = if prefix_length == 0 {
        0
    } else {
        !0u32 << (32 - prefix_length)
    };

    let ip = Ipv4Addr::from(mask_bits);
    Ok(ip.to_string())
}

/// 校验网关是否与本机 IP 位于同一子网
pub fn validate_gateway_in_subnet(
    ip: Ipv4Addr,
    mask: Ipv4Addr,
    gateway: Ipv4Addr,
) -> Result<(), String> {
    let ip_net = u32::from(ip) & u32::from(mask);
    let gw_net = u32::from(gateway) & u32::from(mask);

    if ip_net != gw_net {
        return Err(format!(
            "网关地址 ({}) 与本机 IP ({}) 在掩码 ({}) 下不在同一个子网",
            gateway, ip, mask
        ));
    }

    if ip == gateway {
        return Err("网关地址不能与本机 IP 地址相同".to_string());
    }

    Ok(())
}

/// 比较两个快照是否在网络关键状态上相匹配（用于回滚后现场真值校验） (N-03, N-04)
pub fn verify_snapshot_restored(
    before: &AdapterSnapshot,
    current: &AdapterSnapshot,
) -> Result<(), String> {
    if before.interface_guid != current.interface_guid {
        return Err(format!(
            "网络适配器身份标识不匹配 (期望: {}, 实际: {})",
            before.interface_guid, current.interface_guid
        ));
    }

    if before.dhcp_enabled != current.dhcp_enabled {
        return Err(format!(
            "IP DHCP 状态不匹配 (期望: {}, 实际: {})",
            before.dhcp_enabled, current.dhcp_enabled
        ));
    }

    if before.dns_dhcp_enabled != current.dns_dhcp_enabled {
        return Err(format!(
            "DNS DHCP 状态不匹配 (期望: {}, 实际: {})",
            before.dns_dhcp_enabled, current.dns_dhcp_enabled
        ));
    }

    // 若原配置是静态 IP，则严格检查完整静态 IP 集合与掩码，杜绝辅助 IP 丢失或残留 (N-04)
    if !before.dhcp_enabled {
        if before.addresses.len() != current.addresses.len() {
            return Err(format!(
                "静态 IP 地址数量不匹配 (期望: {} 个 {:?}, 实际: {} 个 {:?})",
                before.addresses.len(),
                before.addresses,
                current.addresses.len(),
                current.addresses
            ));
        }
        for (idx, expected) in before.addresses.iter().enumerate() {
            let actual_match = current.addresses.iter().any(|a| {
                a.ip_address == expected.ip_address && a.prefix_length == expected.prefix_length
            });
            if !actual_match {
                return Err(format!(
                    "原第 {} 项静态 IP ({}/{}) 未在当前配置中恢复 (当前配置: {:?})",
                    idx + 1,
                    expected.ip_address,
                    expected.prefix_length,
                    current.addresses
                ));
            }
        }
        // 严格检查完整默认网关集合，杜绝多网关丢失 (N-04)
        if before.gateways.len() != current.gateways.len() {
            return Err(format!(
                "默认网关数量不匹配 (期望: {} 个 {:?}, 实际: {} 个 {:?})",
                before.gateways.len(),
                before.gateways,
                current.gateways.len(),
                current.gateways
            ));
        }
        for (idx, expected_gw) in before.gateways.iter().enumerate() {
            if !current.gateways.contains(expected_gw) {
                return Err(format!(
                    "原第 {} 项默认网关 ({}) 未在当前配置中恢复 (当前配置: {:?})",
                    idx + 1,
                    expected_gw,
                    current.gateways
                ));
            }
        }
    }

    // 若原配置是静态 DNS，则严格检查静态 DNS 地址完整列表与顺序，杜绝多余 DNS 残留 (N-04)
    if !before.dns_dhcp_enabled {
        if before.dns_servers.len() != current.dns_servers.len() {
            return Err(format!(
                "DNS 服务器列表长度不匹配 (期望: {} 个 {:?}, 实际: {} 个 {:?})",
                before.dns_servers.len(),
                before.dns_servers,
                current.dns_servers.len(),
                current.dns_servers
            ));
        }
        for (idx, expected_dns) in before.dns_servers.iter().enumerate() {
            if current.dns_servers.get(idx) != Some(expected_dns) {
                return Err(format!(
                    "第 {} 位 DNS 服务器不匹配 (期望: {}, 实际: {:?})",
                    idx + 1,
                    expected_dns,
                    current.dns_servers.get(idx)
                ));
            }
        }
    }

    // 核验 IPv6 协议启用状态是否恢复
    if before.ipv6_enabled != current.ipv6_enabled {
        return Err(format!(
            "IPv6 协议绑定状态未恢复 (期望: {:?}, 实际: {})",
            before.ipv6_enabled, current.ipv6_enabled
        ));
    }

    // 若原 IPv6 协议处于启用状态，且当前也处于启用状态，核验 IPv6 详细配置是否恢复
    if before.ipv6_enabled && current.ipv6_enabled {
        if before.ipv6_dhcp_enabled != current.ipv6_dhcp_enabled {
            return Err(format!(
                "IPv6 DHCP 状态未恢复 (期望: {}, 实际: {})",
                before.ipv6_dhcp_enabled, current.ipv6_dhcp_enabled
            ));
        }
        if before.ipv6_dns_dhcp_enabled != current.ipv6_dns_dhcp_enabled {
            return Err(format!(
                "IPv6 DNS DHCP 状态未恢复 (期望: {}, 实际: {})",
                before.ipv6_dns_dhcp_enabled, current.ipv6_dns_dhcp_enabled
            ));
        }
        if !before.ipv6_dhcp_enabled {
            // 核验地址数量完全一致，拒绝额外地址残留 (V6-05)
            if before.ipv6_addresses.len() != current.ipv6_addresses.len() {
                return Err(format!(
                    "IPv6 地址列表数量未恢复 (期望: {}, 实际: {})",
                    before.ipv6_addresses.len(),
                    current.ipv6_addresses.len()
                ));
            }
            for exp in &before.ipv6_addresses {
                if !current.ipv6_addresses.iter().any(|a| {
                    ipv6_addr_eq(&a.ip_address, &exp.ip_address)
                        && a.prefix_length == exp.prefix_length
                }) {
                    return Err(format!(
                        "原静态 IPv6 地址 ({}/{}) 未恢复",
                        exp.ip_address, exp.prefix_length
                    ));
                }
            }
            for cur in &current.ipv6_addresses {
                if !before.ipv6_addresses.iter().any(|a| {
                    ipv6_addr_eq(&a.ip_address, &cur.ip_address)
                        && a.prefix_length == cur.prefix_length
                }) {
                    return Err(format!(
                        "存在残留的未恢复 IPv6 地址 ({}/{})",
                        cur.ip_address, cur.prefix_length
                    ));
                }
            }
        }
        // 核验 IPv6 默认网关列表恢复 (V6-04, V6-05)
        if before.ipv6_gateways.len() != current.ipv6_gateways.len() {
            return Err(format!(
                "IPv6 默认网关数量未恢复 (期望: {}, 实际: {})",
                before.ipv6_gateways.len(),
                current.ipv6_gateways.len()
            ));
        }
        for (idx, exp_gw) in before.ipv6_gateways.iter().enumerate() {
            let cur_gw = current
                .ipv6_gateways
                .get(idx)
                .map(|s| s.as_str())
                .unwrap_or("");
            if !ipv6_addr_eq(exp_gw, cur_gw) {
                return Err(format!(
                    "第 {} 位 IPv6 默认网关未恢复 (期望: {}, 实际: {:?})",
                    idx + 1,
                    exp_gw,
                    current.ipv6_gateways.get(idx)
                ));
            }
        }
        if !before.ipv6_dns_dhcp_enabled {
            if before.ipv6_dns_servers.len() != current.ipv6_dns_servers.len() {
                return Err(format!(
                    "IPv6 DNS 服务器数量未恢复 (期望: {}, 实际: {})",
                    before.ipv6_dns_servers.len(),
                    current.ipv6_dns_servers.len()
                ));
            }
            for (idx, exp_dns) in before.ipv6_dns_servers.iter().enumerate() {
                let cur_dns = current
                    .ipv6_dns_servers
                    .get(idx)
                    .map(|s| s.as_str())
                    .unwrap_or("");
                if !ipv6_addr_eq(exp_dns, cur_dns) {
                    return Err(format!(
                        "第 {} 位 IPv6 DNS 服务器未恢复 (期望: {}, 实际: {:?})",
                        idx + 1,
                        exp_dns,
                        current.ipv6_dns_servers.get(idx)
                    ));
                }
            }
        }
    }

    // 核验 DoH 加密状态是否恢复 (规范化比对模式、模板与回退策略) (N-03)
    let verify_doh_equal = |expected: Option<&DohConfig>,
                            actual: Option<&DohConfig>,
                            label: &str|
     -> Result<(), String> {
        let exp_mode = expected.map(|d| d.mode.as_str()).unwrap_or("off");
        let act_mode = actual.map(|d| d.mode.as_str()).unwrap_or("off");

        if exp_mode == "off" {
            if act_mode != "off" {
                return Err(format!(
                    "{} DoH 加密状态未恢复为关闭 (期望: off, 实际: {})",
                    label, act_mode
                ));
            }
        } else {
            let exp = expected.unwrap();
            let act = match actual {
                Some(a) => a,
                None => {
                    return Err(format!(
                        "{} DoH 状态未恢复 (期望: {:?}, 实际未配置)",
                        label, exp
                    ))
                }
            };
            if exp.mode != act.mode {
                return Err(format!(
                    "{} DoH 模式未恢复 (期望: {}, 实际: {})",
                    label, exp.mode, act.mode
                ));
            }
            if exp.mode == "manual" && exp.template.trim() != act.template.trim() {
                return Err(format!(
                    "{} DoH 模板未恢复 (期望: {}, 实际: {})",
                    label, exp.template, act.template
                ));
            }
            if exp.allow_fallback != act.allow_fallback {
                return Err(format!(
                    "{} DoH 明文回退策略未恢复 (期望: {}, 实际: {})",
                    label, exp.allow_fallback, act.allow_fallback
                ));
            }
        }
        Ok(())
    };

    if before.adapter_doh_settings != current.adapter_doh_settings {
        return Err("单网卡 DoH 条目或原始属性未完整恢复".to_string());
    }
    verify_doh_equal(before.doh1.as_ref(), current.doh1.as_ref(), "首选 DNS")?;
    verify_doh_equal(before.doh2.as_ref(), current.doh2.as_ref(), "备用 DNS")?;

    // 核验系统级全局 DoH 服务器表是否完全恢复 (N-02)
    for (ip, exp) in &before.doh_settings {
        match current.doh_settings.get(ip) {
            None => {
                return Err(format!("原全局 DoH 服务器 ({}) 未在恢复后找到", ip));
            }
            Some(act) => {
                if exp.template.trim() != act.template.trim() {
                    return Err(format!(
                        "全局 DoH 服务器 ({}) 模板未恢复 (期望: {}, 实际: {})",
                        ip, exp.template, act.template
                    ));
                }
                if exp.allow_fallback != act.allow_fallback {
                    return Err(format!(
                        "全局 DoH 服务器 ({}) 明文回退策略未恢复 (期望: {}, 实际: {})",
                        ip, exp.allow_fallback, act.allow_fallback
                    ));
                }
                if exp.auto_upgrade != act.auto_upgrade {
                    return Err(format!(
                        "全局 DoH 服务器 ({}) 自动升级策略未恢复 (期望: {}, 实际: {})",
                        ip, exp.auto_upgrade, act.auto_upgrade
                    ));
                }
            }
        }
    }
    for ip in current.doh_settings.keys() {
        if !before.doh_settings.contains_key(ip) {
            return Err(format!("全局 DoH 服务器列表中存在未清理的残留项 ({})", ip));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_ipv4() {
        assert!(validate_ipv4("192.168.1.1").is_ok());
        assert!(validate_ipv4("0.0.0.0").is_ok());
        assert!(validate_ipv4("255.255.255.255").is_ok());
    }

    #[test]
    fn test_invalid_ipv4_format() {
        assert!(validate_ipv4("").is_err());
        assert!(validate_ipv4("192.168.1").is_err());
        assert!(validate_ipv4("192.168.1.1.1").is_err());
        assert!(validate_ipv4("192.168.1.256").is_err());
        assert!(validate_ipv4("192.168.-1.1").is_err());
        assert!(validate_ipv4("abc.def.ghi.jkl").is_err());
        assert!(validate_ipv4("192.168.1. ").is_err());
    }

    #[test]
    fn test_invalid_ipv4_leading_zeros() {
        assert!(validate_ipv4("192.168.01.1").is_err());
        assert!(validate_ipv4("010.0.0.1").is_err());
        assert!(validate_ipv4("192.168.1.00").is_err());
    }

    #[test]
    fn test_valid_subnet_masks() {
        assert_eq!(validate_subnet_mask("255.255.255.0").unwrap(), 24);
        assert_eq!(validate_subnet_mask("255.255.0.0").unwrap(), 16);
        assert_eq!(validate_subnet_mask("255.0.0.0").unwrap(), 8);
        assert_eq!(validate_subnet_mask("255.255.255.255").unwrap(), 32);
        assert_eq!(validate_subnet_mask("0.0.0.0").unwrap(), 0);
        assert_eq!(validate_subnet_mask("255.255.255.128").unwrap(), 25);
    }

    #[test]
    fn test_invalid_noncontiguous_subnet_masks() {
        assert!(validate_subnet_mask("255.255.255.1").is_err());
        assert!(validate_subnet_mask("255.0.255.0").is_err());
        assert!(validate_subnet_mask("255.255.254.1").is_err());
    }

    #[test]
    fn test_prefix_to_subnet_mask() {
        assert_eq!(prefix_to_subnet_mask(24).unwrap(), "255.255.255.0");
        assert_eq!(prefix_to_subnet_mask(16).unwrap(), "255.255.0.0");
        assert_eq!(prefix_to_subnet_mask(32).unwrap(), "255.255.255.255");
        assert_eq!(prefix_to_subnet_mask(0).unwrap(), "0.0.0.0");
        assert!(prefix_to_subnet_mask(33).is_err());
    }

    #[test]
    fn test_gateway_subnet_validation() {
        let ip = "192.168.1.100".parse::<Ipv4Addr>().unwrap();
        let mask = "255.255.255.0".parse::<Ipv4Addr>().unwrap();

        let gw_valid = "192.168.1.1".parse::<Ipv4Addr>().unwrap();
        assert!(validate_gateway_in_subnet(ip, mask, gw_valid).is_ok());

        let gw_diff_subnet = "192.168.2.1".parse::<Ipv4Addr>().unwrap();
        assert!(validate_gateway_in_subnet(ip, mask, gw_diff_subnet).is_err());

        assert!(validate_gateway_in_subnet(ip, mask, ip).is_err());
    }

    #[test]
    fn test_validate_dns_combination() {
        assert!(validate_dns_combination("", "").is_ok());
        assert!(validate_dns_combination("8.8.8.8", "").is_ok());
        assert!(validate_dns_combination("8.8.8.8", "8.8.4.4").is_ok());
        assert!(validate_dns_combination("", "8.8.4.4").is_err());
        assert!(validate_dns_combination("invalid", "").is_err());
        assert!(validate_dns_combination("8.8.8.8", "invalid").is_err());
    }

    #[test]
    fn test_validate_doh_config() {
        assert!(validate_doh_config(
            "",
            Some(&DohConfig {
                mode: "off".into(),
                template: "".into(),
                allow_fallback: true
            }),
            "首选 DNS"
        )
        .is_ok());
        assert!(validate_doh_config("", None, "首选 DNS").is_ok());

        assert!(validate_doh_config(
            "",
            Some(&DohConfig {
                mode: "manual".into(),
                template: "https://doh.pub/dns-query".into(),
                allow_fallback: true
            }),
            "首选 DNS"
        )
        .is_err());
        assert!(validate_doh_config(
            "223.5.5.5",
            Some(&DohConfig {
                mode: "auto".into(),
                template: "".into(),
                allow_fallback: true
            }),
            "首选 DNS"
        )
        .is_ok());
        assert!(validate_doh_config(
            "223.5.5.5",
            Some(&DohConfig {
                mode: "manual".into(),
                template: "https://dns.alidns.com/dns-query".into(),
                allow_fallback: true
            }),
            "首选 DNS"
        )
        .is_ok());

        assert!(validate_doh_config(
            "223.5.5.5",
            Some(&DohConfig {
                mode: "manual".into(),
                template: "".into(),
                allow_fallback: true
            }),
            "首选 DNS"
        )
        .is_err());
        assert!(validate_doh_config(
            "223.5.5.5",
            Some(&DohConfig {
                mode: "manual".into(),
                template: "http://doh.pub/dns-query".into(),
                allow_fallback: true
            }),
            "首选 DNS"
        )
        .is_err());
        assert!(validate_doh_config(
            "223.5.5.5",
            Some(&DohConfig {
                mode: "manual".into(),
                template: "https://doh".into(),
                allow_fallback: true
            }),
            "首选 DNS"
        )
        .is_err());
    }

    #[test]
    fn test_verify_snapshot_restored() {
        let before = AdapterSnapshot {
            adapter_name: "Eth".into(),
            interface_index: 1,
            interface_guid: "guid-123".into(),
            status: "Up".into(),
            dhcp_enabled: false,
            dns_dhcp_enabled: false,
            addresses: vec![
                Ipv4AddressConfig {
                    ip_address: "192.168.1.10".into(),
                    prefix_length: 24,
                    mask: "255.255.255.0".into(),
                },
                Ipv4AddressConfig {
                    ip_address: "192.168.1.11".into(),
                    prefix_length: 24,
                    mask: "255.255.255.0".into(),
                },
            ],
            gateways: vec!["192.168.1.1".into(), "192.168.1.254".into()],
            dns_servers: vec!["8.8.8.8".into(), "1.1.1.1".into()],
            ip: "192.168.1.10".into(),
            mask: "255.255.255.0".into(),
            gateway: "192.168.1.1".into(),
            dns1: "8.8.8.8".into(),
            dns2: "1.1.1.1".into(),
            doh1: Some(DohConfig {
                mode: "manual".into(),
                template: "https://dns.google/dns-query".into(),
                allow_fallback: true,
            }),
            doh2: None,
            ipv6_enabled: true,
            ipv6_dhcp_enabled: true,
            ipv6_dns_dhcp_enabled: true,
            ipv6_mode: Some("dhcp".into()),
            ipv6_dns_mode: Some("dhcp".into()),
            ipv6_addresses: vec![],
            ipv6_gateways: vec![],
            ipv6_dns_servers: vec![],
            ipv6_ip: "".into(),
            ipv6_prefix: Some(64),
            ipv6_gateway: "".into(),
            ipv6_dns1: "".into(),
            ipv6_dns2: "".into(),
            doh_settings: HashMap::new(),
            has_interface_doh: false,
            adapter_doh_settings: HashMap::new(),
            doh_supported: true,
        };

        let mut matching = before.clone();
        assert!(verify_snapshot_restored(&before, &matching).is_ok());

        // 1. 首选 IP 未恢复
        matching.addresses[0].ip_address = "192.168.1.20".into();
        assert!(verify_snapshot_restored(&before, &matching).is_err());

        // 2. 缺少第二个辅助 IP (N-04)
        let mut missing_sec_ip = before.clone();
        missing_sec_ip.addresses.pop();
        assert!(verify_snapshot_restored(&before, &missing_sec_ip).is_err());

        // 3. 缺少第二个网关 (N-04)
        let mut missing_sec_gw = before.clone();
        missing_sec_gw.gateways.pop();
        assert!(verify_snapshot_restored(&before, &missing_sec_gw).is_err());

        // 4. 多出一个 DNS 服务器 (N-04)
        let mut extra_dns = before.clone();
        extra_dns.dns_servers.push("114.114.114.114".into());
        assert!(verify_snapshot_restored(&before, &extra_dns).is_err());

        // 5. DNS 顺序颠倒
        let mut order_wrong = before.clone();
        order_wrong.dns_servers = vec!["1.1.1.1".into(), "8.8.8.8".into()];
        assert!(verify_snapshot_restored(&before, &order_wrong).is_err());

        // 6. GUID 不一致
        let mut guid_wrong = before.clone();
        guid_wrong.interface_guid = "other-guid".into();
        assert!(verify_snapshot_restored(&before, &guid_wrong).is_err());

        // 7. DoH 模板不匹配 (N-03)
        let mut doh_tmpl_wrong = before.clone();
        doh_tmpl_wrong.doh1 = Some(DohConfig {
            mode: "manual".into(),
            template: "https://other.example/dns-query".into(),
            allow_fallback: true,
        });
        assert!(verify_snapshot_restored(&before, &doh_tmpl_wrong).is_err());

        // 8. DoH 回退策略不匹配 (N-03)
        let mut doh_fb_wrong = before.clone();
        doh_fb_wrong.doh1 = Some(DohConfig {
            mode: "manual".into(),
            template: "https://dns.google/dns-query".into(),
            allow_fallback: false,
        });
        assert!(verify_snapshot_restored(&before, &doh_fb_wrong).is_err());

        // 9. 期望 off 但实际为 manual (N-03)
        let mut exp_off = before.clone();
        exp_off.doh1 = Some(DohConfig {
            mode: "off".into(),
            template: "".into(),
            allow_fallback: true,
        });
        let mut act_manual = before.clone();
        act_manual.doh1 = Some(DohConfig {
            mode: "manual".into(),
            template: "https://doh.pub/dns-query".into(),
            allow_fallback: true,
        });
        assert!(verify_snapshot_restored(&exp_off, &act_manual).is_err());

        // 10. 恢复后残留了非原有的全局 DoH 服务器 (N-02)
        let mut extra_global_doh = before.clone();
        extra_global_doh.doh_settings.insert(
            "198.51.100.53".into(),
            DohServerSetting {
                template: "https://residual.example/dns-query".into(),
                allow_fallback: true,
                auto_upgrade: true,
            },
        );
        assert!(verify_snapshot_restored(&before, &extra_global_doh).is_err());

        // 11. 原全局 DoH 服务器未在恢复后找到 (N-02)
        let mut before_with_doh = before.clone();
        before_with_doh.doh_settings.insert(
            "198.51.100.53".into(),
            DohServerSetting {
                template: "https://expected.example/dns-query".into(),
                allow_fallback: false,
                auto_upgrade: false,
            },
        );
        assert!(verify_snapshot_restored(&before_with_doh, &before).is_err());

        // 12. IPv6 DHCP 状态未恢复
        let mut v6_dhcp_wrong = before.clone();
        v6_dhcp_wrong.ipv6_dhcp_enabled = false;
        assert!(verify_snapshot_restored(&before, &v6_dhcp_wrong).is_err());

        // 13. 静态 IPv6 地址未恢复
        let mut before_v6_static = before.clone();
        before_v6_static.ipv6_dhcp_enabled = false;
        before_v6_static.ipv6_ip = "2001:db8::1".into();
        before_v6_static.ipv6_addresses = vec![Ipv6AddressConfig {
            ip_address: "2001:db8::1".into(),
            prefix_length: 64,
            prefix_origin: "Manual".into(),
            suffix_origin: "Manual".into(),
        }];
        let mut act_v6_missing = before_v6_static.clone();
        act_v6_missing.ipv6_addresses = vec![];
        assert!(verify_snapshot_restored(&before_v6_static, &act_v6_missing).is_err());

        // 14. IPv6 DNS 未恢复
        let mut before_v6_dns = before.clone();
        before_v6_dns.ipv6_dns_dhcp_enabled = false;
        before_v6_dns.ipv6_dns_servers = vec!["2400:3200::1".into()];
        let mut act_v6_dns_empty = before_v6_dns.clone();
        act_v6_dns_empty.ipv6_dns_servers = vec![];
        assert!(verify_snapshot_restored(&before_v6_dns, &act_v6_dns_empty).is_err());

        // 15. IPv6 恢复时存在残留额外静态地址或网关不一致 (V6-05)
        let mut act_v6_extra = before_v6_static.clone();
        act_v6_extra.ipv6_addresses.push(Ipv6AddressConfig {
            ip_address: "2001:db8::99".into(),
            prefix_length: 64,
            prefix_origin: "Manual".into(),
            suffix_origin: "Manual".into(),
        });
        assert!(verify_snapshot_restored(&before_v6_static, &act_v6_extra).is_err());

        let mut before_v6_gw = before_v6_static.clone();
        before_v6_gw.ipv6_gateways = vec!["fe80::1".into()];
        let mut act_v6_wrong_gw = before_v6_gw.clone();
        act_v6_wrong_gw.ipv6_gateways = vec!["fe80::bad".into()];
        assert!(verify_snapshot_restored(&before_v6_gw, &act_v6_wrong_gw).is_err());

        // 16. 等价 IPv6 规范化比较应通过 (V6-07)
        let mut act_v6_equiv = before_v6_gw.clone();
        act_v6_equiv.ipv6_addresses = vec![Ipv6AddressConfig {
            ip_address: "2001:0db8:0000:0000:0000:0000:0000:0001".into(),
            prefix_length: 64,
            prefix_origin: "Manual".into(),
            suffix_origin: "Manual".into(),
        }];
        assert!(verify_snapshot_restored(&before_v6_gw, &act_v6_equiv).is_ok());

        // 17. IPv6 DNS 服务器数量不一致拦截 (R6-04)
        let mut before_v6_2dns = before_v6_static.clone();
        before_v6_2dns.ipv6_dns_dhcp_enabled = false;
        before_v6_2dns.ipv6_dns_servers = vec!["2001:db8::54".into(), "2001:db8::55".into()];
        let mut act_v6_3dns = before_v6_2dns.clone();
        act_v6_3dns.ipv6_dns_servers.push("2001:db8::56".into());
        assert!(verify_snapshot_restored(&before_v6_2dns, &act_v6_3dns).is_err());

        // RR6-01: 自动接口中的手动地址也必须恢复前缀及来源。
        let mut mixed = before_v6_static.clone();
        mixed.ipv6_dhcp_enabled = true;
        let touched = vec!["2001:db8::1".to_string()];
        let mut replaced = mixed.clone();
        replaced.ipv6_addresses[0].prefix_length = 80;
        assert!(verify_ipv6_address_rollback(&mixed, &replaced, &touched).is_err());
        assert!(verify_ipv6_address_rollback(&mixed, &mixed, &touched).is_ok());
        replaced.ipv6_addresses.clear();
        assert!(verify_ipv6_address_rollback(&mixed, &replaced, &touched).is_err());

        // 同 IP、同前缀但自动来源被手动替换也必须失败。
        let mut automatic = mixed.clone();
        automatic.ipv6_addresses[0].prefix_origin = "RouterAdvertisement".into();
        automatic.ipv6_addresses[0].suffix_origin = "Random".into();
        assert!(verify_ipv6_address_rollback(&automatic, &mixed, &touched).is_err());
        assert!(verify_ipv6_address_rollback(&automatic, &automatic, &touched).is_ok());
        // 自动地址消失/重新分配不能被误判为手动配置残留。
        assert!(verify_ipv6_address_rollback(&automatic, &replaced, &touched).is_ok());
        replaced.ipv6_addresses = automatic.ipv6_addresses.clone();
        replaced.ipv6_addresses[0].ip_address = "2001:db8::abcd".into();
        assert!(verify_ipv6_address_rollback(&automatic, &replaced, &touched).is_ok());
        let mut unknown = mixed.clone();
        unknown.ipv6_addresses[0].prefix_origin.clear();
        assert!(verify_ipv6_address_rollback(&unknown, &mixed, &touched).is_err());
        assert!(verify_ipv6_address_rollback(&automatic, &unknown, &touched).is_err());
    }

    #[test]
    fn test_ipv4_config_mode_defaults_and_deserialization() {
        // 兼容旧版不传 ip_mode / dns_mode 的 JSON (N-07)
        let json_old = r#"{
            "adapter": "Ethernet",
            "ip": "192.168.1.100",
            "mask": "255.255.255.0"
        }"#;
        let cfg_old: Ipv4Config = serde_json::from_str(json_old).unwrap();
        assert_eq!(cfg_old.ip_mode, None);
        assert_eq!(cfg_old.dns_mode, None);
        assert_eq!(cfg_old.ipv6_mode, None);
        assert_eq!(cfg_old.ipv6_dns_mode, None);

        // 新版显式传递 dhcp 模式与 ipv6 (N-07)
        let json_new = r#"{
            "adapter": "Ethernet",
            "ipMode": "dhcp",
            "ip": "",
            "mask": "",
            "dnsMode": "static",
            "dns1": "223.5.5.5",
            "dns2": "223.6.6.6",
            "ipv6Mode": "static",
            "ipv6Ip": "2001:db8::1",
            "ipv6Prefix": 64,
            "ipv6Gateway": "fe80::1",
            "ipv6DnsMode": "static",
            "ipv6Dns1": "2400:3200::1",
            "ipv6Dns2": "2400:3200:baba::1"
        }"#;
        let cfg_new: Ipv4Config = serde_json::from_str(json_new).unwrap();
        assert_eq!(cfg_new.ip_mode, Some("dhcp".to_string()));
        assert_eq!(cfg_new.dns_mode, Some("static".to_string()));
        assert_eq!(cfg_new.ipv6_mode, Some("static".to_string()));
        assert_eq!(cfg_new.ipv6_ip, "2001:db8::1");
        assert_eq!(cfg_new.ipv6_prefix, Some(64));
        assert_eq!(cfg_new.ipv6_gateway, "fe80::1");
        assert_eq!(cfg_new.ipv6_dns_mode, Some("static".to_string()));
        assert_eq!(cfg_new.ipv6_dns1, "2400:3200::1");
        assert_eq!(cfg_new.ipv6_dns2, "2400:3200:baba::1");
    }

    #[test]
    fn test_ipv6_validators() {
        // validate_ipv6
        assert!(validate_ipv6("2001:db8::1").is_ok());
        assert!(validate_ipv6("::1").is_ok());
        assert!(validate_ipv6("fe80::1%12").is_ok());
        assert!(validate_ipv6("fe80::1").is_ok());
        assert!(validate_ipv6("2001:db8:85a3:0:0:8a2e:370:7334").is_ok());
        assert!(validate_ipv6("invalid_ip").is_err());
        assert!(validate_ipv6("192.168.1.1").is_err());

        // validate_ipv6_prefix
        assert!(validate_ipv6_prefix(64).is_ok());
        assert!(validate_ipv6_prefix(1).is_ok());
        assert!(validate_ipv6_prefix(128).is_ok());
        assert!(validate_ipv6_prefix(0).is_err());
        assert!(validate_ipv6_prefix(129).is_err());

        // validate_ipv6_dns_combination
        assert!(validate_ipv6_dns_combination("", "").is_err());
        assert!(validate_ipv6_dns_combination("  ", "\t").is_err());
        assert!(validate_ipv6_dns_combination("2400:3200::1", "").is_ok());
        assert!(validate_ipv6_dns_combination("2400:3200::1", "2400:3200:baba::1").is_ok());
        assert!(validate_ipv6_dns_combination("", "2400:3200::1").is_err());
        assert!(validate_ipv6_dns_combination("invalid", "").is_err());

        // ipv6_addr_eq (V6-07)
        assert!(ipv6_addr_eq("2001:db8::1", "2001:0db8:0:0:0:0:0:1"));
        assert!(ipv6_addr_eq("2001:db8::20", "2001:0db8:0:0:0:0:0:20"));
        assert!(ipv6_addr_eq("fe80::1%12", "FE80::1%12"));
        assert!(ipv6_addr_eq("::1", "0:0:0:0:0:0:0:1"));
        assert!(!ipv6_addr_eq("2001:db8::1", "2001:db8::2"));
        assert!(!ipv6_addr_eq("fe80::1%12", "fe80::1%13"));
    }
}
