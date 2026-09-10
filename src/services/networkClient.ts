import { invoke } from '@tauri-apps/api/tauri';
import type { AdapterInfo, AdapterSnapshot, Ipv4Config, OperationResult } from '../types/network';

/**
 * 原生网络客户端服务
 * 
 * 封装前端与 Tauri 后端 Rust 进程之间的类型安全 IPC 调用，
 * 隔离底层通信细节，对外提供标准异步 Promise 接口。
 */
export const networkClient = {
  /**
   * 扫描并获取当前 Windows 系统中所有的网络适配器列表
   * 
   * 调用后端 `get_network_adapters` 命令，枚举本地所有以太网、WLAN 等物理或虚拟网卡。
   * 
   * @returns Promise<AdapterInfo[]> 网络适配器信息数组
   */
  async getNetworkAdapters(): Promise<AdapterInfo[]> {
    return await invoke<AdapterInfo[]>('get_network_adapters');
  },

  /**
   * 获取指定网络适配器的全息运行时快照
   * 
   * 调用后端 `get_current_config` 命令，采集目标网卡当前的：
   * - IPv4 地址、子网掩码、默认网关
   * - IPv4 DNS 服务器列表及对应的 DoH 加密状态
   * - IPv6 协议组件启用状态
   * - IPv6 地址、前缀长度、IPv6 默认网关
   * - IPv6 首选与备用 DNS 服务器列表及 DHCP 租约状态
   * 
   * @param adapterName 目标网卡接口名称 (如 "以太网", "WLAN")
   * @returns Promise<AdapterSnapshot> 适配器快照对象
   */
  async getCurrentConfig(adapterName: string): Promise<AdapterSnapshot> {
    return await invoke<AdapterSnapshot>('get_current_config', {
      adapterName,
    });
  },

  /**
   * 向系统应用指定的网络配置 (含 IPv4 / IPv6 / DNS / DoH)
   * 
   * 调用后端 `apply_adapter_ipv4_config` 命令，执行带原子快照与读回校验的事务变更。
   * 若底层发生任何配置失败或读回不匹配，后端将自动安全回滚至修改前快照。
   * 
   * @param cfg 包含目标网卡、IP 分配模式、IPv6 模式及 DNS 等全量配置参数
   * @returns Promise<OperationResult> 操作执行结果（包含是否成功、回滚标识、回滚提示及最新快照）
   */
  async applyAdapterIpv4Config(cfg: Ipv4Config): Promise<OperationResult> {
    return await invoke<OperationResult>('apply_adapter_ipv4_config', {
      cfg,
    });
  },
};
