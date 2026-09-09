use std::net::Ipv4Addr;

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

/// DNS over HTTPS (DoH) 配置项
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DohConfig {
    pub mode: String,          // "off" | "auto" | "manual"
    #[serde(default)]
    pub template: String,      // DoH 模板 URL (如 https://doh.pub/dns-query)
    #[serde(default)]
    pub allow_fallback: bool,  // 失败时是否回退使用未加密请求
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
}

fn default_true() -> bool {
    true
}

/// 前端提交的 IPv4 配置意图
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Ipv4Config {
    pub adapter: String,
    pub ip: String,
    pub mask: String,
    #[serde(default)]
    pub gateway: String,
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
}

/// 校验 DoH 配置
pub fn validate_doh_config(dns_ip: &str, doh: Option<&DohConfig>, label: &str) -> Result<(), String> {
    let Some(cfg) = doh else {
        return Ok(());
    };

    let mode = cfg.mode.trim().to_lowercase();
    if mode == "off" || mode.is_empty() {
        return Ok(());
    }

    if dns_ip.trim().is_empty() {
        return Err(format!("启用了{}的 DNS over HTTPS，但未配置该 DNS 服务器的 IP 地址", label));
    }

    if mode == "manual" {
        let t = cfg.template.trim();
        if t.is_empty() {
            return Err(format!("{}已选择手动模板模式，必须填写 DoH 模板 URL", label));
        }
        if !t.starts_with("https://") {
            return Err(format!("{}的 DoH 模板必须是以 https:// 开头的合法 URL", label));
        }
        if t.len() < 10 || !t[8..].contains('/') {
            return Err(format!("{}的 DoH 模板格式不完整 (如 https://doh.pub/dns-query)", label));
        }
    } else if mode != "auto" {
        return Err(format!("{}的 DoH 模式 '{}' 不受支持 (仅支持 off / auto / manual)", label, cfg.mode));
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

/// 比较两个快照是否在网络关键状态上相匹配（用于回滚后现场真值校验）
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

    // 若原配置是静态 IP，则检查静态 IP 地址与掩码
    if !before.dhcp_enabled {
        if let Some(expected_first) = before.addresses.first() {
            let actual_match = current.addresses.iter().any(|a| {
                a.ip_address == expected_first.ip_address
                    && a.prefix_length == expected_first.prefix_length
            });
            if !actual_match {
                return Err(format!(
                    "原首选 IP ({}/{}) 未在当前配置中恢复 (当前配置: {:?})",
                    expected_first.ip_address, expected_first.prefix_length, current.addresses
                ));
            }
        }
        // 检查网关
        if let Some(expected_gw) = before.gateways.first() {
            if !current.gateways.iter().any(|g| g == expected_gw) {
                return Err(format!(
                    "原默认网关 ({}) 未在当前配置中恢复 (当前配置: {:?})",
                    expected_gw, current.gateways
                ));
            }
        }
    }

    // 若原配置是静态 DNS，则检查静态 DNS 地址与顺序
    if !before.dns_dhcp_enabled {
        if before.dns_servers.len() > current.dns_servers.len() {
            return Err(format!(
                "DNS 服务器恢复不完整 (期望: {:?}, 实际: {:?})",
                before.dns_servers, current.dns_servers
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

    // 核验 DoH 加密状态是否恢复
    if before.doh1 != current.doh1 {
        return Err(format!(
            "首选 DNS DoH 状态未恢复 (期望: {:?}, 实际: {:?})",
            before.doh1, current.doh1
        ));
    }
    if before.doh2 != current.doh2 {
        return Err(format!(
            "备用 DNS DoH 状态未恢复 (期望: {:?}, 实际: {:?})",
            before.doh2, current.doh2
        ));
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
    fn test_invalid_ipv4_leading_zeros() {
        assert!(validate_ipv4("010.0.0.1").is_err());
        assert!(validate_ipv4("192.168.01.1").is_err());
    }

    #[test]
    fn test_invalid_ipv4_format() {
        assert!(validate_ipv4("").is_err());
        assert!(validate_ipv4("192.168.1").is_err());
        assert!(validate_ipv4("192.168.1.1.1").is_err());
        assert!(validate_ipv4("256.0.0.1").is_err());
        assert!(validate_ipv4("abc.def.ghi.jkl").is_err());
    }

    #[test]
    fn test_valid_subnet_masks() {
        assert_eq!(validate_subnet_mask("255.255.255.0").unwrap(), 24);
        assert_eq!(validate_subnet_mask("255.255.0.0").unwrap(), 16);
        assert_eq!(validate_subnet_mask("255.0.0.0").unwrap(), 8);
        assert_eq!(validate_subnet_mask("255.255.255.255").unwrap(), 32);
        assert_eq!(validate_subnet_mask("255.255.255.252").unwrap(), 30);
        assert_eq!(validate_subnet_mask("0.0.0.0").unwrap(), 0);
    }

    #[test]
    fn test_invalid_noncontiguous_subnet_masks() {
        // A-06 明确要求拒绝 255.0.255.0
        assert!(validate_subnet_mask("255.0.255.0").is_err());
        assert!(validate_subnet_mask("255.255.0.255").is_err());
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
        let ip = "192.168.1.100".parse().unwrap();
        let mask = "255.255.255.0".parse().unwrap();
        let valid_gw = "192.168.1.1".parse().unwrap();
        let invalid_gw = "192.168.2.1".parse().unwrap();

        assert!(validate_gateway_in_subnet(ip, mask, valid_gw).is_ok());
        assert!(validate_gateway_in_subnet(ip, mask, invalid_gw).is_err());
        assert!(validate_gateway_in_subnet(ip, mask, ip).is_err());
    }

    #[test]
    fn test_validate_dns_combination() {
        assert!(validate_dns_combination("", "").is_ok());
        assert!(validate_dns_combination("8.8.8.8", "").is_ok());
        assert!(validate_dns_combination("8.8.8.8", "1.1.1.1").is_ok());
        // 拒绝单独指定 dns2 而未配置 dns1
        assert!(validate_dns_combination("", "1.1.1.1").is_err());
        // 非法 IP 格式检查
        assert!(validate_dns_combination("invalid-ip", "").is_err());
        assert!(validate_dns_combination("8.8.8.8", "invalid-ip").is_err());
    }

    #[test]
    fn test_validate_doh_config() {
        // 关闭状态无需校验
        assert!(validate_doh_config("", Some(&DohConfig { mode: "off".into(), template: "".into(), allow_fallback: true }), "首选 DNS").is_ok());
        assert!(validate_doh_config("", None, "首选 DNS").is_ok());

        // 开启 DoH 但未提供 DNS IP
        assert!(validate_doh_config("", Some(&DohConfig { mode: "manual".into(), template: "https://doh.pub/dns-query".into(), allow_fallback: true }), "首选 DNS").is_err());

        // 开启自动模式
        assert!(validate_doh_config("223.5.5.5", Some(&DohConfig { mode: "auto".into(), template: "".into(), allow_fallback: true }), "首选 DNS").is_ok());

        // 手动模式合法 URL
        assert!(validate_doh_config("223.5.5.5", Some(&DohConfig { mode: "manual".into(), template: "https://dns.alidns.com/dns-query".into(), allow_fallback: true }), "首选 DNS").is_ok());

        // 手动模式非法 URL (非 https，缺少路径等)
        assert!(validate_doh_config("223.5.5.5", Some(&DohConfig { mode: "manual".into(), template: "".into(), allow_fallback: true }), "首选 DNS").is_err());
        assert!(validate_doh_config("223.5.5.5", Some(&DohConfig { mode: "manual".into(), template: "http://doh.pub/dns-query".into(), allow_fallback: true }), "首选 DNS").is_err());
        assert!(validate_doh_config("223.5.5.5", Some(&DohConfig { mode: "manual".into(), template: "https://doh".into(), allow_fallback: true }), "首选 DNS").is_err());
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
            addresses: vec![Ipv4AddressConfig {
                ip_address: "192.168.1.10".into(),
                prefix_length: 24,
                mask: "255.255.255.0".into(),
            }],
            gateways: vec!["192.168.1.1".into()],
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
        };

        let mut matching = before.clone();
        assert!(verify_snapshot_restored(&before, &matching).is_ok());

        // IP 未恢复
        matching.addresses[0].ip_address = "192.168.1.20".into();
        assert!(verify_snapshot_restored(&before, &matching).is_err());

        // DNS 顺序颠倒
        let mut order_wrong = before.clone();
        order_wrong.dns_servers = vec!["1.1.1.1".into(), "8.8.8.8".into()];
        assert!(verify_snapshot_restored(&before, &order_wrong).is_err());

        // GUID 不一致
        let mut guid_wrong = before.clone();
        guid_wrong.interface_guid = "other-guid".into();
        assert!(verify_snapshot_restored(&before, &guid_wrong).is_err());
    }
}
