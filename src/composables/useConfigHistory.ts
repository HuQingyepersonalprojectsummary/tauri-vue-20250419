import { ref } from 'vue';
import type { HistoryItem, Ipv4Config } from '../types/network';

const CONFIG_KEY = 'net_config_history_v1';
const LEGACY_KEY = 'net_config_history';
const MAX_HISTORY_ITEMS = 10;

export function useConfigHistory() {
  const configList = ref<HistoryItem[]>([]);
  const storageWarning = ref<string | null>(null);

  /**
   * 严格校验单条历史记录是否符合规范 (A-10, R-09)
   */
  function isValidHistoryItem(item: unknown): item is HistoryItem {
    if (!item || typeof item !== 'object') {
      return false;
    }
    const record = item as Record<string, unknown>;

    // 必填字段严格校验
    if (typeof record.adapter !== 'string' || record.adapter.trim().length === 0) return false;
    if (typeof record.ip !== 'string' || record.ip.trim().length === 0) return false;
    if (typeof record.mask !== 'string' || record.mask.trim().length === 0) return false;

    // 可选字段类型严格校验：若存在必须为 string，防止传入非字符串抛错导致整批清空 (R-09)
    if (record.gateway !== undefined && record.gateway !== null && typeof record.gateway !== 'string') return false;
    if (record.dns1 !== undefined && record.dns1 !== null && typeof record.dns1 !== 'string') return false;
    if (record.dns2 !== undefined && record.dns2 !== null && typeof record.dns2 !== 'string') return false;

    // 版本字段校验：仅接受当前明确支持的版本 1，未知未来版本、负数或小数均严格拒绝 (R-09, F-08)
    if (record.schemaVersion !== undefined && record.schemaVersion !== 1) {
      return false;
    }

    // 适配器名称长度限制
    const adapterStr = record.adapter.trim();
    if (adapterStr.length === 0 || adapterStr.length > 256) return false;

    // IPv4 地址与掩码基本点分十进制格式安全校验 (F-08)
    const ipv4Regex = /^(?:(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]\d|\d)\.){3}(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]\d|\d)$/;
    if (!ipv4Regex.test(record.ip.trim())) return false;
    if (!ipv4Regex.test(record.mask.trim())) return false;

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

    if (record.doh1 !== undefined && record.doh1 !== null) {
      if (typeof record.doh1 !== 'object') return false;
      const d1 = record.doh1 as Record<string, unknown>;
      if (d1.mode !== 'off' && d1.mode !== 'auto' && d1.mode !== 'manual') return false;
      if (typeof d1.template !== 'string') return false;
      if (typeof d1.allowFallback !== 'boolean') return false;
    }

    if (record.doh2 !== undefined && record.doh2 !== null) {
      if (typeof record.doh2 !== 'object') return false;
      const d2 = record.doh2 as Record<string, unknown>;
      if (d2.mode !== 'off' && d2.mode !== 'auto' && d2.mode !== 'manual') return false;
      if (typeof d2.template !== 'string') return false;
      if (typeof d2.allowFallback !== 'boolean') return false;
    }

    if (record.ipv6Enabled !== undefined && record.ipv6Enabled !== null && typeof record.ipv6Enabled !== 'boolean') {
      return false;
    }

    return true;
  }

  /**
   * 从 localStorage 规范化加载历史，截断超额记录，过滤损坏数据 (A-10, R-09)
   */
  function loadConfigList(): void {
    storageWarning.value = null;
    let needPersistMigration = false;

    try {
      let raw = localStorage.getItem(CONFIG_KEY);
      // 迁移旧版数据
      if (!raw) {
        const legacy = localStorage.getItem(LEGACY_KEY);
        if (legacy) {
          raw = legacy;
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

      const validItems: HistoryItem[] = [];
      let hasMalformed = false;

      for (const item of parsed) {
        try {
          if (isValidHistoryItem(item)) {
            validItems.push({
              schemaVersion: 1,
              id: typeof item.id === 'string' && item.id.trim() ? item.id : `${Date.now()}_${Math.random().toString(36).substring(2, 8)}`,
              timestamp: typeof item.timestamp === 'number' ? item.timestamp : Date.now(),
              adapter: item.adapter.trim(),
              ip: item.ip.trim(),
              mask: item.mask.trim(),
              gateway: typeof item.gateway === 'string' ? item.gateway.trim() : '',
              dns1: typeof item.dns1 === 'string' ? item.dns1.trim() : '',
              dns2: typeof item.dns2 === 'string' ? item.dns2.trim() : '',
              doh1: item.doh1 ? {
                mode: item.doh1.mode,
                template: item.doh1.template,
                allowFallback: item.doh1.allowFallback,
              } : undefined,
              doh2: item.doh2 ? {
                mode: item.doh2.mode,
                template: item.doh2.template,
                allowFallback: item.doh2.allowFallback,
              } : undefined,
              ipv6Enabled: typeof item.ipv6Enabled === 'boolean' ? item.ipv6Enabled : undefined,
            });
          } else {
            hasMalformed = true;
          }
        } catch {
          hasMalformed = true;
        }
      }

      if (hasMalformed) {
        storageWarning.value = '部分历史记录数据格式不兼容，已自动过滤';
      }

      // 严格限制最大数量为 10
      configList.value = validItems.slice(0, MAX_HISTORY_ITEMS);

      // 旧数据格式成功迁移后，原子写入新 key 并清理旧 key (R-09, F-08)
      if (needPersistMigration && validItems.length > 0) {
        try {
          localStorage.setItem(CONFIG_KEY, JSON.stringify(configList.value));
          localStorage.removeItem(LEGACY_KEY);
        } catch (e) {
          storageWarning.value = '旧版历史记录迁移持久化失败: ' + (e instanceof Error ? e.message : String(e));
        }
      }
    } catch {
      storageWarning.value = '加载历史记录失败，已重置为空';
      configList.value = [];
    }
  }

  /**
   * 安全保存配置至历史记录，发生存储异常时仅抛出警告，不阻塞网络成功流 (A-09)
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
    };

    // 检查是否已有相同配置项（根据字段比对）
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
        item.ipv6Enabled === newItem.ipv6Enabled
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
   * 删除指定的单条历史 (R-09)
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
