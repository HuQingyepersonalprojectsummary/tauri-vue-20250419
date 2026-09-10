export interface AdapterInfo {
  name: string;
  status: string;
  rawStatus?: string;
  displayName?: string;
  interfaceIndex?: number;
  interfaceGuid?: string;
  macAddress?: string;
}

export interface Ipv4AddressConfig {
  ipAddress: string;
  prefixLength: number;
  mask: string;
}

export interface Ipv6AddressConfig {
  ipAddress: string;
  prefixLength: number;
}

export interface DohConfig {
  mode: 'off' | 'auto' | 'manual';
  template: string;
  allowFallback: boolean;
}

export interface AdapterSnapshot {
  adapterName: string;
  interfaceIndex: number;
  interfaceGuid: string;
  status: string;
  dhcpEnabled: boolean;
  dnsDhcpEnabled?: boolean;
  addresses: Ipv4AddressConfig[];
  gateways: string[];
  dnsServers: string[];
  ip: string;
  mask: string;
  gateway: string;
  dns1: string;
  dns2: string;
  doh1?: DohConfig;
  doh2?: DohConfig;
  ipv6Enabled?: boolean;
  ipv6DhcpEnabled?: boolean;
  ipv6DnsDhcpEnabled?: boolean;
  ipv6Addresses?: Ipv6AddressConfig[];
  ipv6Gateways?: string[];
  ipv6DnsServers?: string[];
  ipv6Ip?: string;
  ipv6Prefix?: number;
  ipv6Gateway?: string;
  ipv6Dns1?: string;
  ipv6Dns2?: string;
}

export interface Ipv4Config {
  adapter: string;
  ip: string;
  mask: string;
  gateway: string;
  dns1: string;
  dns2: string;
  doh1?: DohConfig;
  doh2?: DohConfig;
  ipv6Enabled?: boolean;
  ipMode?: 'dhcp' | 'static' | 'keep';
  dnsMode?: 'dhcp' | 'static' | 'keep';
  ipv6Mode?: 'dhcp' | 'static' | 'keep';
  ipv6Ip?: string;
  ipv6Prefix?: number | string;
  ipv6Gateway?: string;
  ipv6DnsMode?: 'dhcp' | 'static' | 'keep';
  ipv6Dns1?: string;
  ipv6Dns2?: string;
}

export interface OperationResult {
  success: boolean;
  message: string;
  rolledBack: boolean;
  rollbackMessage?: string;
  snapshot?: AdapterSnapshot;
}

export interface HistoryItem {
  schemaVersion: number;
  id: string;
  timestamp: number;
  adapter: string;
  ip: string;
  mask: string;
  gateway: string;
  dns1: string;
  dns2: string;
  doh1?: DohConfig;
  doh2?: DohConfig;
  ipv6Enabled?: boolean;
  ipMode?: 'dhcp' | 'static' | 'keep';
  dnsMode?: 'dhcp' | 'static' | 'keep';
  ipv6Mode?: 'dhcp' | 'static' | 'keep';
  ipv6Ip?: string;
  ipv6Prefix?: number | string;
  ipv6Gateway?: string;
  ipv6DnsMode?: 'dhcp' | 'static' | 'keep';
  ipv6Dns1?: string;
  ipv6Dns2?: string;
}

