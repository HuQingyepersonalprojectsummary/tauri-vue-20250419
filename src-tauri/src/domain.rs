use std::collections::HashMap;
use std::net::{Ipv4Addr, Ipv6Addr};

/// 网络适配器概要信息
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AdapterInfo {
    pub name: String,
    pub status: String,
    pub raw_status: String,
    pub display_name: String,
    pub interface_index: u32,
    pub interface_guid: String,
    pub mac_address: Option<String>,
}

/// 单个 IPv4 地址及掩码信息
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Ipv4AddressConfig {
    pub ip_address: String,
    pub prefix_length: u8,
    pub mask: String,
}

/// 单个 IPv6 地址及前缀长度信息
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Ipv6AddressConfig {
    pub ip_address: String,
    pub prefix_length: u8,
}

/// DNS over HTTPS (DoH) 配置项
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DohConfig {
    pub mode: String, // "off" | "auto" | "manual"
    #[serde(default)]
    pub template: String, // DoH 模板 URL (如 https://doh.pub/dns-query)
    #[serde(default)]
    pub allow_fallback: bool, // 失败时是否回退使用未加密请求
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

/// 系统级全局 DoH 服务器条目配置 (N-02)
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DohServerSetting {
    #[serde(default)]
    pub template: String,
    #[serde(default)]
    pub allow_fallback: bool,
    #[serde(default)]
    pub auto_upgrade: bool,
}

/// 适配器完整快照（同时提供便捷的一维字段以保证兼容性）
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AdapterSnapshot {
    pub adapter_name: String,
    pub interface_index: u32,
    pub interface_guid: String,
    pub status: String,
    pub dhcp_enabled: bool,
    #[serde(default)]
    pub dns_dhcp_enabled: bool,
    pub addresses: Vec<Ipv4AddressConfig>,
    pub gateways: Vec<String>,
    pub dns_servers: Vec<String>,
    // 兼容原 Ipv4Config 字段
    pub ip: String,
    pub mask: String,
    pub gateway: String,
    pub dns1: String,
    pub dns2: String,
    // DoH 与 IPv6 扩展字段
    #[serde(default)]
    pub doh1: Option<DohConfig>,
    #[serde(default)]
    pub doh2: Option<DohConfig>,
    #[serde(default = "default_true")]
    pub ipv6_enabled: bool,
    // 系统级全局已配置的 DoH 服务器表 (N-02)
    #[serde(default)]
    pub doh_settings: HashMap<String, DohServerSetting>,
    #[serde(default = "default_true")]
    pub doh_supported: bool,
    // IPv6 扩展字段
    #[serde(default)]
    pub ipv6_mode: Option<String>,
    #[serde(default)]
    pub ipv6_ip: String,
    #[serde(default)]
    pub ipv6_prefix: Option<u8>,
    #[serde(default)]
    pub ipv6_gateway: String,
    #[serde(default)]
    pub ipv6_dns_mode: Option<String>,
    #[serde(default)]
    pub ipv6_dns1: String,
    #[serde(default)]
    pub ipv6_dns2: String,
    #[serde(default)]
    pub ipv6_addresses: Vec<Ipv6AddressConfig>,
    #[serde(default)]
    pub ipv6_gateways: Vec<String>,
    #[serde(default)]
    pub ipv6_dns_servers: Vec<String>,
    #[serde(default = "default_true")]
    pub ipv6_dhcp_enabled: bool,
    #[serde(default = "default_true")]
    pub ipv6_dns_dhcp_enabled: bool,
}

fn default_true() -> bool {
    true
}

/// 前端提交的 IPv4 配置意图 (支持显式协议意图: "dhcp" | "static" | "keep") (N-07)
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Ipv4Config {
    pub adapter: String,
    #[serde(default)]
    pub ip_mode: Option<String>, // "dhcp" | "static" | "keep"
    #[serde(default)]
    pub ip: String,
    #[serde(default)]
    pub mask: String,
    #[serde(default)]
    pub gateway: String,
    #[serde(default)]
    pub dns_mode: Option<String>, // "dhcp" | "static" | "keep"
    #[serde(default)]
    pub dns1: String,
    #[serde(default)]
    pub dns2: String,
    #[serde(default)]
    pub doh1: Option<DohConfig>,
    #[serde(default)]
    pub doh2: Option<DohConfig>,
    #[serde(default)]
    pub ipv6_enabled: Option<bool>,
    #[serde(default)]
    pub ipv6_mode: Option<String>, // "dhcp" | "static" | "keep"
    #[serde(default)]
    pub ipv6_ip: String,
    #[serde(default)]
    pub ipv6_prefix: Option<u8>,
    #[serde(default)]
    pub ipv6_gateway: String,
    #[serde(default)]
    pub ipv6_dns_mode: Option<String>, // "dhcp" | "static" | "keep"
    #[serde(default)]
    pub ipv6_dns1: String,
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

/// 校验 IPv6 DNS 配置组合（不允许未配置首选 DNS 而单独配置辅助 DNS）
pub fn validate_ipv6_dns_combination(dns1: &str, dns2: &str) -> Result<(), String> {
    let d1 = dns1.trim();
    let d2 = dns2.trim();
    if d1.is_empty() && !d2.is_empty() {
        return Err("若配置 IPv6 辅助 DNS，必须先配置 IPv6 首选 DNS (DNS1)".to_string());
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
        if !before.ipv6_dhcp_enabled && !before.ipv6_addresses.is_empty() {
            for exp in &before.ipv6_addresses {
                if !current.ipv6_addresses.iter().any(|a| {
                    a.ip_address.eq_ignore_ascii_case(&exp.ip_address)
                        && a.prefix_length == exp.prefix_length
                }) {
                    return Err(format!(
                        "原静态 IPv6 地址 ({}/{}) 未恢复",
                        exp.ip_address, exp.prefix_length
                    ));
                }
            }
        }
        if !before.ipv6_dns_dhcp_enabled
            && !before.ipv6_dns_servers.is_empty()
            && before.ipv6_dns_servers != current.ipv6_dns_servers
        {
            return Err(format!(
                "IPv6 DNS 服务器列表未恢复 (期望: {:?}, 实际: {:?})",
                before.ipv6_dns_servers, current.ipv6_dns_servers
            ));
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
        assert!(validate_ipv6_dns_combination("", "").is_ok());
        assert!(validate_ipv6_dns_combination("2400:3200::1", "").is_ok());
        assert!(validate_ipv6_dns_combination("2400:3200::1", "2400:3200:baba::1").is_ok());
        assert!(validate_ipv6_dns_combination("", "2400:3200::1").is_err());
        assert!(validate_ipv6_dns_combination("invalid", "").is_err());
    }
}
