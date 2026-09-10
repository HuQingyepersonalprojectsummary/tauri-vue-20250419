import { ref } from 'vue';
import type { HistoryItem, Ipv4Config } from '../types/network';

/** LocalStorage 存储主键 (版本 1) */
const CONFIG_KEY = 'net_config_history_v1';
/** 旧版本数据存储主键 (用于无缝平滑迁移) */
const LEGACY_KEY = 'net_config_history';
/** 最大历史保存条数限制，防止无限膨胀 */
const MAX_HISTORY_ITEMS = 10;

/**
 * 网络配置历史记录组合式函数
 * 
 * 职责：
 * 1. 管理用户已保存或应用成功的历史网络配置记录（上限 10 条）；
 * 2. 具备结构化数据校验（Schema Version 1），单条损坏自动隔离清理，不影响其余条目；
 * 3. 存储异常时产生独立警告（storageWarning），绝不阻断网络配置应用主流程。
 */
export function useConfigHistory() {
  /** 历史配置列表响应式引用 */
  const configList = ref<HistoryItem[]>([]);
  /** 本地存储异常提示信息 (为 null 表示正常) */
  const storageWarning = ref<string | null>(null);

  /**
   * 严格校验单条历史记录数据结构的完整性与类型合法性 (A-10, R-09)
   * 
   * 检查必填项、字符串有效性、版本号及正则格式，防止被非法篡改或脏数据破坏界面。
   * 
   * @param item 待校验的历史对象
   * @returns boolean 是否为合法的 HistoryItem
   */
  function isValidHistoryItem(item: unknown): item is HistoryItem {
    if (!item || typeof item !== 'object') {
      return false;
    }
    const record = item as Record<string, unknown>;

    // 必填字段严格校验
    if (typeof record.adapter !== 'string') return false;
    const adapterStr = record.adapter.trim();
    if (adapterStr.length === 0 || adapterStr.length > 256) return false;

    if (typeof record.ip !== 'string') return false;
    if (typeof record.mask !== 'string') return false;

    // 可选字段类型严格校验：若存在必须为 string，防止传入非字符串抛错导致整批清空 (R-09)
    if (record.gateway !== undefined && record.gateway !== null && typeof record.gateway !== 'string') return false;
    if (record.dns1 !== undefined && record.dns1 !== null && typeof record.dns1 !== 'string') return false;
    if (record.dns2 !== undefined && record.dns2 !== null && typeof record.dns2 !== 'string') return false;

    // 版本字段校验：仅接受当前明确支持的版本 1，未知未来版本、负数或小数均严格拒绝 (R-09, F-08)
    if (record.schemaVersion !== undefined && record.schemaVersion !== 1) {
      return false;
    }

    // IPv4 地址与掩码基本点分十进制格式安全校验 (F-08, V6-09)
    const ipv4Regex = /^(?:(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]\d|\d)\.){3}(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]\d|\d)$/;
    const ipTrimmed = record.ip.trim();
    const maskTrimmed = record.mask.trim();

    // 当 IPv4 处于静态模式或旧版隐式静态模式（含非空 IP）时，IPv4 地址与掩码为必填
    if (record.ipMode === 'static' || (record.ipMode === undefined && (!record.ipv6Mode || record.ipv6Mode === 'dhcp' || record.ipv6Mode === 'keep') && ipTrimmed.length > 0)) {
      if (ipTrimmed.length === 0 || !ipv4Regex.test(ipTrimmed)) return false;
      if (maskTrimmed.length === 0 || !ipv4Regex.test(maskTrimmed)) return false;
    } else {
      if (ipTrimmed.length > 0 && !ipv4Regex.test(ipTrimmed)) return false;
      if (maskTrimmed.length > 0 && !ipv4Regex.test(maskTrimmed)) return false;
    }

    // 可选字段格式与长度校验
    if (record.gateway !== undefined && record.gateway !== null) {
      if (typeof record.gateway !== 'string') return false;
      const gw = record.gateway.trim();
      if (gw.length > 0 && !ipv4Regex.test(gw)) return false;
    }
    if (record.dns1 !== undefined && record.dns1 !== null) {
      if (typeof record.dns1 !== 'string') return false;
      const d1 = record.dns1.trim();
      if (d1.length > 0 && !ipv4Regex.test(d1)) return false;
    }
    if (record.dns2 !== undefined && record.dns2 !== null) {
      if (typeof record.dns2 !== 'string') return false;
      const d2 = record.dns2.trim();
      if (d2.length > 0 && !ipv4Regex.test(d2)) return false;
    }

    // DoH 首选加密结构校验
    if (record.doh1 !== undefined && record.doh1 !== null) {
      if (typeof record.doh1 !== 'object') return false;
      const d1 = record.doh1 as Record<string, unknown>;
      if (d1.mode !== 'off' && d1.mode !== 'auto' && d1.mode !== 'manual') return false;
      if (typeof d1.template !== 'string') return false;
      if (typeof d1.allowFallback !== 'boolean') return false;
    }

    // DoH 备用加密结构校验
    if (record.doh2 !== undefined && record.doh2 !== null) {
      if (typeof record.doh2 !== 'object') return false;
      const d2 = record.doh2 as Record<string, unknown>;
      if (d2.mode !== 'off' && d2.mode !== 'auto' && d2.mode !== 'manual') return false;
      if (typeof d2.template !== 'string') return false;
      if (typeof d2.allowFallback !== 'boolean') return false;
    }

    // IPv6 协议组件开关校验
    if (record.ipv6Enabled !== undefined && record.ipv6Enabled !== null && typeof record.ipv6Enabled !== 'boolean') {
      return false;
    }

    // IP 与 DNS 分配模式合法性校验
    if (record.ipMode !== undefined && record.ipMode !== null) {
      if (record.ipMode !== 'dhcp' && record.ipMode !== 'static' && record.ipMode !== 'keep') return false;
    }
    if (record.dnsMode !== undefined && record.dnsMode !== null) {
      if (record.dnsMode !== 'dhcp' && record.dnsMode !== 'static' && record.dnsMode !== 'keep') return false;
    }

    // IPv6 地址分配与 DNS 分配模式合法性校验
    if (record.ipv6Mode !== undefined && record.ipv6Mode !== null) {
      if (record.ipv6Mode !== 'dhcp' && record.ipv6Mode !== 'static' && record.ipv6Mode !== 'keep') return false;
    }
    if (record.ipv6DnsMode !== undefined && record.ipv6DnsMode !== null) {
      if (record.ipv6DnsMode !== 'dhcp' && record.ipv6DnsMode !== 'static' && record.ipv6DnsMode !== 'keep') return false;
    }
    if (record.ipv6Ip !== undefined && record.ipv6Ip !== null && typeof record.ipv6Ip !== 'string') return false;
    if (record.ipv6Prefix !== undefined && record.ipv6Prefix !== null && typeof record.ipv6Prefix !== 'number' && typeof record.ipv6Prefix !== 'string') return false;
    if (record.ipv6Gateway !== undefined && record.ipv6Gateway !== null && typeof record.ipv6Gateway !== 'string') return false;
    if (record.ipv6Dns1 !== undefined && record.ipv6Dns1 !== null && typeof record.ipv6Dns1 !== 'string') return false;
    if (record.ipv6Dns2 !== undefined && record.ipv6Dns2 !== null && typeof record.ipv6Dns2 !== 'string') return false;

    return true;
  }

  /**
   * 从 LocalStorage 安全加载并规范化历史记录 (A-10, R-09)
   * 
   * 执行逻辑：
   * 1. 优先读取版本 1 数据，若不存在则平滑迁移旧格式数据；
   * 2. 逐条执行严格合法性检查，自动剔除损坏异常数据；
   * 3. 限制至多 10 条最新记录，完成迁移后持久化清洗后的有效记录。
   */
  function loadConfigList(): void {
    storageWarning.value = null;
    let needPersistMigration = false;

    try {
      let raw = localStorage.getItem(CONFIG_KEY);
      // 兼容迁移旧版历史数据
      if (!raw) {
        const legacyRaw = localStorage.getItem(LEGACY_KEY);
        if (legacyRaw) {
          raw = legacyRaw;
          needPersistMigration = true;
        }
      }

      if (!raw) {
        configList.value = [];
        return;
      }

      const parsed = JSON.parse(raw);
      if (!Array.isArray(parsed)) {
        configList.value = [];
        return;
      }

      const validList: HistoryItem[] = [];
      let corruptedCount = 0;

      for (const item of parsed) {
        if (isValidHistoryItem(item)) {
          validList.push({
            schemaVersion: 1,
            id: typeof item.id === 'string' && item.id.trim() ? item.id.trim() : `${Date.now()}_${Math.random().toString(36).substring(2, 8)}`,
            timestamp: typeof item.timestamp === 'number' && !isNaN(item.timestamp) ? item.timestamp : Date.now(),
            adapter: item.adapter.trim(),
            ip: item.ip.trim(),
            mask: item.mask.trim(),
            gateway: (item.gateway || '').trim(),
            dns1: (item.dns1 || '').trim(),
            dns2: (item.dns2 || '').trim(),
            doh1: item.doh1 ? { mode: item.doh1.mode, template: item.doh1.template.trim(), allowFallback: item.doh1.allowFallback } : undefined,
            doh2: item.doh2 ? { mode: item.doh2.mode, template: item.doh2.template.trim(), allowFallback: item.doh2.allowFallback } : undefined,
            ipv6Enabled: typeof item.ipv6Enabled === 'boolean' ? item.ipv6Enabled : undefined,
            ipMode: item.ipMode || 'static',
            dnsMode: item.dnsMode || 'static',
            ipv6Mode: item.ipv6Mode,
            ipv6Ip: (item.ipv6Ip || '').trim(),
            ipv6Prefix: item.ipv6Prefix,
            ipv6Gateway: (item.ipv6Gateway || '').trim(),
            ipv6DnsMode: item.ipv6DnsMode,
            ipv6Dns1: (item.ipv6Dns1 || '').trim(),
            ipv6Dns2: (item.ipv6Dns2 || '').trim(),
          });
        } else {
          corruptedCount++;
        }
      }

      if (corruptedCount > 0) {
        storageWarning.value = `部分历史记录数据格式不兼容，已自动过滤 ${corruptedCount} 条异常记录`;
        needPersistMigration = true;
      }

      // 严格限制最大数量为 10 条
      configList.value = validList.slice(0, MAX_HISTORY_ITEMS);

      // 旧数据格式成功迁移或发生清洗后，原子写入新 key 并清理旧 key (R-09, F-08)
      if (needPersistMigration) {
        try {
          localStorage.setItem(CONFIG_KEY, JSON.stringify(configList.value));
          localStorage.removeItem(LEGACY_KEY);
        } catch {
          // 降级静默，不抛出阻断异常
        }
      }
    } catch {
      storageWarning.value = '历史配置读取失败，已自动重置历史记录列表';
      configList.value = [];
    }
  }

  /**
   * 安全保存配置至历史记录 (A-09)
   * 
   * 执行逻辑：
   * 1. 深度比对当前所有字段，若已有完全相同的配置则将其移至队首（去重更新）；
   * 2. 插入新记录并截断至上限 10 条；
   * 3. 捕获 LocalStorage 容量超限或禁用异常，仅提示警告，绝不影响网络成功状态。
   * 
   * @param cfg 待保存的网络配置对象
   * @returns { saved: boolean, warning?: string } 保存结果与警告信息
   */
  function saveConfig(cfg: Ipv4Config): { saved: boolean; warning?: string } {
    storageWarning.value = null;

    const newItem: HistoryItem = {
      schemaVersion: 1,
      id: `${Date.now()}_${Math.random().toString(36).substring(2, 8)}`,
      timestamp: Date.now(),
      adapter: cfg.adapter.trim(),
      ip: cfg.ip.trim(),
      mask: cfg.mask.trim(),
      gateway: cfg.gateway.trim(),
      dns1: cfg.dns1.trim(),
      dns2: cfg.dns2.trim(),
      doh1: cfg.doh1 ? { ...cfg.doh1 } : undefined,
      doh2: cfg.doh2 ? { ...cfg.doh2 } : undefined,
      ipv6Enabled: typeof cfg.ipv6Enabled === 'boolean' ? cfg.ipv6Enabled : undefined,
      ipMode: cfg.ipMode,
      dnsMode: cfg.dnsMode,
      ipv6Mode: cfg.ipv6Mode,
      ipv6Ip: cfg.ipv6Ip?.trim(),
      ipv6Prefix: cfg.ipv6Prefix,
      ipv6Gateway: cfg.ipv6Gateway?.trim(),
      ipv6DnsMode: cfg.ipv6DnsMode,
      ipv6Dns1: cfg.ipv6Dns1?.trim(),
      ipv6Dns2: cfg.ipv6Dns2?.trim(),
    };

    // 检查是否已有完全相同的配置项（深度比较全量字段）
    const existingIndex = configList.value.findIndex(
      item =>
        item.adapter === newItem.adapter &&
        item.ip === newItem.ip &&
        item.mask === newItem.mask &&
        item.gateway === newItem.gateway &&
        item.dns1 === newItem.dns1 &&
        item.dns2 === newItem.dns2 &&
        JSON.stringify(item.doh1) === JSON.stringify(newItem.doh1) &&
        JSON.stringify(item.doh2) === JSON.stringify(newItem.doh2) &&
        item.ipv6Enabled === newItem.ipv6Enabled &&
        item.ipMode === newItem.ipMode &&
        item.dnsMode === newItem.dnsMode &&
        item.ipv6Mode === newItem.ipv6Mode &&
        item.ipv6Ip === newItem.ipv6Ip &&
        item.ipv6Prefix === newItem.ipv6Prefix &&
        item.ipv6Gateway === newItem.ipv6Gateway &&
        item.ipv6DnsMode === newItem.ipv6DnsMode &&
        item.ipv6Dns1 === newItem.ipv6Dns1 &&
        item.ipv6Dns2 === newItem.ipv6Dns2
    );

    if (existingIndex !== -1) {
      configList.value.splice(existingIndex, 1);
    }

    configList.value.unshift(newItem);

    if (configList.value.length > MAX_HISTORY_ITEMS) {
      configList.value = configList.value.slice(0, MAX_HISTORY_ITEMS);
    }

    try {
      localStorage.setItem(CONFIG_KEY, JSON.stringify(configList.value));
      return { saved: true };
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      storageWarning.value = `本地存储失败 (${msg})，未保留至历史记录`;
      return { saved: false, warning: storageWarning.value };
    }
  }

  /**
   * 删除指定的单条历史记录 (R-09)
   * 
   * @param id 目标历史记录的唯一 ID
   */
  function removeConfig(id: string): void {
    configList.value = configList.value.filter(item => item.id !== id);
    try {
      localStorage.setItem(CONFIG_KEY, JSON.stringify(configList.value));
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      storageWarning.value = `删除历史记录本地同步失败: ${msg}`;
    }
  }

  return {
    configList,
    storageWarning,
    loadConfigList,
    saveConfig,
    removeConfig,
  };
}
