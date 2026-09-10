/**
 * 网络适配器基础信息
 */
export interface AdapterInfo {
  /** 适配器接口别名 (例如: "以太网", "WLAN") */
  name: string;
  /** 用户友好的状态展示字符串 */
  status: string;
  /** 系统底层返回的原始状态 (例如: "Up", "Disconnected") */
  rawStatus?: string;
  /** 网卡硬件设备友好名称 (例如: "Intel(R) Wi-Fi 6 AX201 160MHz") */
  displayName?: string;
  /** 网卡接口数字索引 (ifIndex) */
  interfaceIndex?: number;
  /** 适配器全局唯一标识符 GUID */
  interfaceGuid?: string;
  /** 网卡 MAC 物理硬件地址 */
  macAddress?: string;
}

/**
 * IPv4 单个地址及掩码配置
 */
export interface Ipv4AddressConfig {
  /** IPv4 地址 (点分十进制字符串) */
  ipAddress: string;
  /** 网络前缀长度 (0..=32，如 24 代表 255.255.255.0) */
  prefixLength: number;
  /** 子网掩码点分十进制字符串 (例如: "255.255.255.0") */
  mask: string;
}

/**
 * IPv6 单个地址及前缀配置
 */
export interface Ipv6AddressConfig {
  /** IPv6 地址字符串 (支持标准冒号十六进制及压缩格式) */
  ipAddress: string;
  /** 网络前缀长度 (1..=128，通常为 64) */
  prefixLength: number;
  /** Windows 地址来源；旧快照可缺失，不能据此推断为手动地址 */
  prefixOrigin?: string;
  suffixOrigin?: string;
}

/**
 * DNS over HTTPS (DoH) 加密解析配置
 */
export interface DohConfig {
  /** DoH 加密模式: off=关闭, auto=开启自动识别, manual=手动指定模板 */
  mode: 'off' | 'auto' | 'manual';
  /** DoH 解析服务的 HTTPS 模板 URL (例如: "https://dns.alidns.com/dns-query") */
  template: string;
  /** 当 DoH 失败或超时时，是否允许降级回退到常规未加密请求 (平滑容灾) */
  allowFallback: boolean;
}

/**
 * 网络适配器运行时完整快照 (用于前端展示、差异比对及事务性安全回滚)
 */
export interface AdapterSnapshot {
  /** 目标网卡名称 */
  adapterName: string;
  /** 接口数字索引 */
  interfaceIndex: number;
  /** 接口唯一 GUID */
  interfaceGuid: string;
  /** 当前连接状态 */
  status: string;
  /** IPv4 是否启用了 DHCP 自动获取 */
  dhcpEnabled: boolean;
  /** IPv4 DNS 是否由 DHCP 自动分配 */
  dnsDhcpEnabled?: boolean;
  /** 当前适配器绑定的全部 IPv4 地址列表 */
  addresses: Ipv4AddressConfig[];
  /** 当前生效的 IPv4 默认网关列表 */
  gateways: string[];
  /** 当前生效的 IPv4 DNS 服务器列表 (索引 0 为首选，1 为备用) */
  dnsServers: string[];
  /** 主 IPv4 地址 (通常为首个单播地址) */
  ip: string;
  /** 主 IPv4 子网掩码 */
  mask: string;
  /** 首选 IPv4 默认网关 */
  gateway: string;
  /** 首选 IPv4 DNS 服务器 */
  dns1: string;
  /** 备用 IPv4 DNS 服务器 */
  dns2: string;
  /** 首选 IPv4 DNS 绑定的 DoH 加密策略 */
  doh1?: DohConfig;
  /** 备用 IPv4 DNS 绑定的 DoH 加密策略 */
  doh2?: DohConfig;
  /** 此适配器上 IPv6 协议组件 (ms_tcpip6) 是否处于启用状态 */
  ipv6Enabled?: boolean;
  /** IPv6 地址是否由 DHCPv6 / 路由器通告 (SLAAC) 自动分配 */
  ipv6DhcpEnabled?: boolean;
  /** IPv6 DNS 是否由 DHCPv6 自动下发 */
  ipv6DnsDhcpEnabled?: boolean;
  /** 当前适配器绑定的全部 IPv6 地址列表 */
  ipv6Addresses?: Ipv6AddressConfig[];
  /** 当前生效的 IPv6 默认网关列表 */
  ipv6Gateways?: string[];
  /** 当前生效的 IPv6 DNS 服务器列表 (索引 0 为首选，1 为备用) */
  ipv6DnsServers?: string[];
  /** 主 IPv6 地址 (单播) */
  ipv6Ip?: string;
  /** 主 IPv6 前缀长度 (默认 64) */
  ipv6Prefix?: number;
  /** 默认 IPv6 网关 */
  ipv6Gateway?: string;
  /** 首选 IPv6 DNS 服务器 (例如: 2400:3200::1) */
  ipv6Dns1?: string;
  /** 备用 IPv6 DNS 服务器 (例如: 2400:3200:baba::1) */
  ipv6Dns2?: string;
}

/**
 * 待应用的网络配置对象 (提交至后端的配置变更参数)
 */
export interface Ipv4Config {
  /** 目标网卡名称 */
  adapter: string;
  /** IPv4 地址 (静态模式必填，DHCP 模式留空或忽略) */
  ip: string;
  /** IPv4 子网掩码 (静态模式必填，如 255.255.255.0) */
  mask: string;
  /** IPv4 默认网关 (可选) */
  gateway: string;
  /** 首选 IPv4 DNS 服务器 (静态模式可选) */
  dns1: string;
  /** 备用 IPv4 DNS 服务器 (静态模式可选) */
  dns2: string;
  /** 首选 IPv4 DNS 对应的 DoH 配置 */
  doh1?: DohConfig;
  /** 备用 IPv4 DNS 对应的 DoH 配置 */
  doh2?: DohConfig;
  /** 是否启用此网卡的 IPv6 协议组件 (undefined 表示保持系统现状) */
  ipv6Enabled?: boolean;
  /** IPv4 地址分配模式: dhcp=自动获取, static=静态配置, keep=保持原样 */
  ipMode?: 'dhcp' | 'static' | 'keep';
  /** IPv4 DNS 分配模式: dhcp=自动获取, static=静态配置, keep=保持原样 */
  dnsMode?: 'dhcp' | 'static' | 'keep';
  /** IPv6 地址分配模式: dhcp=自动获取 (SLAAC/DHCPv6), static=静态配置, keep=保持原样 */
  ipv6Mode?: 'dhcp' | 'static' | 'keep';
  /** 静态 IPv6 地址 */
  ipv6Ip?: string;
  /** 静态 IPv6 前缀长度 (1..=128，默认 64) */
  ipv6Prefix?: number | string;
  /** 静态 IPv6 默认网关 */
  ipv6Gateway?: string;
  /** IPv6 DNS 分配模式: dhcp=自动获取, static=静态配置, keep=保持原样 */
  ipv6DnsMode?: 'dhcp' | 'static' | 'keep';
  /** 首选 IPv6 DNS 服务器 */
  ipv6Dns1?: string;
  /** 备用 IPv6 DNS 服务器 */
  ipv6Dns2?: string;
}

/**
 * 后端配置应用操作的执行结果
 */
export interface OperationResult {
  /** 操作是否全部成功且通过了读回深度校验 */
  success: boolean;
  /** 操作反馈信息说明 */
  message: string;
  /** 是否因检测到失败或校验不匹配而触发了自动安全回滚 */
  rolledBack: boolean;
  /** 回滚详细状态提示信息 */
  rollbackMessage?: string;
  /** 最终生效的最新网络快照 */
  snapshot?: AdapterSnapshot;
}

/**
 * 保存在本地 LocalStorage 的历史配置条目
 */
export interface HistoryItem {
  /** 数据结构版本号 (用于向下兼容与安全隔离) */
  schemaVersion: number;
  /** 唯一历史 ID (通常基于时间戳生成) */
  id: string;
  /** 记录创建时间戳 (毫秒) */
  timestamp: number;
  /** 适用的网卡名称 */
  adapter: string;
  /** IPv4 地址 */
  ip: string;
  /** IPv4 子网掩码 */
  mask: string;
  /** IPv4 网关 */
  gateway: string;
  /** 首选 IPv4 DNS */
  dns1: string;
  /** 备用 IPv4 DNS */
  dns2: string;
  /** 首选 DoH 配置 */
  doh1?: DohConfig;
  /** 备用 DoH 配置 */
  doh2?: DohConfig;
  /** IPv6 组件开关状态 */
  ipv6Enabled?: boolean;
  /** IPv4 地址模式 */
  ipMode?: 'dhcp' | 'static' | 'keep';
  /** IPv4 DNS 模式 */
  dnsMode?: 'dhcp' | 'static' | 'keep';
  /** IPv6 地址模式 */
  ipv6Mode?: 'dhcp' | 'static' | 'keep';
  /** 静态 IPv6 地址 */
  ipv6Ip?: string;
  /** 静态 IPv6 前缀长度 */
  ipv6Prefix?: number | string;
  /** 静态 IPv6 网关 */
  ipv6Gateway?: string;
  /** IPv6 DNS 模式 */
  ipv6DnsMode?: 'dhcp' | 'static' | 'keep';
  /** 首选 IPv6 DNS */
  ipv6Dns1?: string;
  /** 备用 IPv6 DNS */
  ipv6Dns2?: string;
}
