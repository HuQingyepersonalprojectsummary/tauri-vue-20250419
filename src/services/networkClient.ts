import { invoke } from '@tauri-apps/api/tauri';
import type { AdapterInfo, AdapterSnapshot, Ipv4Config, OperationResult } from '../types/network';

export const networkClient = {
  /**
   * 获取系统中所有的网络适配器列表
   */
  async getNetworkAdapters(): Promise<AdapterInfo[]> {
    return await invoke<AdapterInfo[]>('get_network_adapters');
  },

  /**
   * 获取指定网络适配器的完整快照
   */
  async getCurrentConfig(adapterName: string): Promise<AdapterSnapshot> {
    return await invoke<AdapterSnapshot>('get_current_config', {
      adapterName,
    });
  },

  /**
   * 应用指定的 IPv4 配置并获得执行结果（含回滚信息）
   */
  async applyAdapterIpv4Config(cfg: Ipv4Config): Promise<OperationResult> {
    return await invoke<OperationResult>('apply_adapter_ipv4_config', {
      cfg,
    });
  },
};
