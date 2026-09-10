import { ref, reactive, watch } from 'vue';
import type { AdapterInfo, AdapterSnapshot, Ipv4Config, OperationResult } from '../types/network';
import { networkClient } from '../services/networkClient';
import {
  validateIpAddress,
  validateSubnetMask,
  validateGatewayInSubnet,
  validateIpv6Address,
  validateIpv6Prefix,
} from '../utils/validation';

export function useNetworkConfig(onConfigApplied?: (cfg: Ipv4Config) => void) {
  const adapters = ref<AdapterInfo[]>([]);
  const selectedAdapter = ref('');
  const isLoading = ref(false);
  const statusMsg = ref('');
  const statusType = ref<'info' | 'success' | 'warning' | 'error'>('info');
  const currentSnapshot = ref<AdapterSnapshot | null>(null);

  // 表单输入
  const ipConfig = reactive<Ipv4Config>({
    adapter: '',
    ip: '',
    mask: '255.255.255.0',
    gateway: '',
    dns1: '',
    dns2: '',
    doh1: {
      mode: 'off',
      template: '',
      allowFallback: true,
    },
    doh2: {
      mode: 'off',
      template: '',
      allowFallback: true,
    },
    ipv6Enabled: true,
    ipMode: 'static',
    dnsMode: 'static',
    ipv6Mode: 'dhcp',
    ipv6Ip: '',
    ipv6Prefix: 64,
    ipv6Gateway: '',
    ipv6DnsMode: 'dhcp',
    ipv6Dns1: '',
    ipv6Dns2: '',
  });

  // 每个适配器的独立草稿缓存，防止网卡切换时的配置串写 (A-08, R-04)
  const adapterDrafts = new Map<string, Ipv4Config>();

  // 标记当前是否正在受控载入历史，防止 watcher 异步调度引发草稿串写 (R-04)
  let isApplyingHistory = false;
  let requestCounter = 0;

  /**
   * 清空表单字段至初始状态
   */
  function clearFormFields(adapterName: string): void {
    ipConfig.adapter = adapterName;
    ipConfig.ip = '';
    ipConfig.mask = '255.255.255.0';
    ipConfig.gateway = '';
    ipConfig.dns1 = '';
    ipConfig.dns2 = '';
    ipConfig.doh1 = {
      mode: 'off',
      template: '',
      allowFallback: true,
    };
    ipConfig.doh2 = {
      mode: 'off',
      template: '',
      allowFallback: true,
    };
    ipConfig.ipv6Enabled = true;
    ipConfig.ipMode = 'static';
    ipConfig.dnsMode = 'static';
    ipConfig.ipv6Mode = 'dhcp';
    ipConfig.ipv6Ip = '';
    ipConfig.ipv6Prefix = 64;
    ipConfig.ipv6Gateway = '';
    ipConfig.ipv6DnsMode = 'dhcp';
    ipConfig.ipv6Dns1 = '';
    ipConfig.ipv6Dns2 = '';
    currentSnapshot.value = null;
  }

  /**
   * 加载所有网络适配器
   */
  async function loadAdapters(): Promise<void> {
    isLoading.value = true;
    statusMsg.value = '正在获取网络适配器列表...';
    statusType.value = 'info';

    try {
      const list = await networkClient.getNetworkAdapters();
      adapters.value = list;
      if (list.length > 0) {
        if (!selectedAdapter.value || !list.some(a => a.name === selectedAdapter.value)) {
          selectedAdapter.value = list[0].name;
        }
      } else {
        selectedAdapter.value = '';
        statusMsg.value = '未检测到可用的网络适配器';
        statusType.value = 'warning';
      }
    } catch (err: unknown) {
      statusMsg.value = '获取适配器列表失败: ' + (err instanceof Error ? err.message : String(err));
      statusType.value = 'error';
    } finally {
      isLoading.value = false;
    }
  }

  /**
   * 获取指定网卡的当前实际配置
   */
  async function fetchCurrentConfig(targetAdapter: string): Promise<void> {
    if (!targetAdapter) return;
    const reqId = ++requestCounter;

    isLoading.value = true;
    statusMsg.value = `正在获取 [${targetAdapter}] 的网络配置...`;
    statusType.value = 'info';

    try {
      const snapshot = await networkClient.getCurrentConfig(targetAdapter);
      // 若当前用户已切换到其他网卡，丢弃陈旧请求结果，防止竞态覆盖 (R-04)
      if (reqId !== requestCounter || selectedAdapter.value !== targetAdapter) {
        return;
      }
      currentSnapshot.value = snapshot;

      // 填充表单
      ipConfig.adapter = targetAdapter;
      ipConfig.ip = snapshot.ip || '';
      ipConfig.mask = snapshot.mask || '255.255.255.0';
      ipConfig.gateway = snapshot.gateway || '';
      ipConfig.dns1 = snapshot.dns1 || '';
      ipConfig.dns2 = snapshot.dns2 || '';
      ipConfig.doh1 = snapshot.doh1
        ? { ...snapshot.doh1 }
        : { mode: 'off', template: '', allowFallback: true };
      ipConfig.doh2 = snapshot.doh2
        ? { ...snapshot.doh2 }
        : { mode: 'off', template: '', allowFallback: true };
      ipConfig.ipv6Enabled = typeof snapshot.ipv6Enabled === 'boolean' ? snapshot.ipv6Enabled : true;
      ipConfig.ipMode = snapshot.dhcpEnabled ? 'dhcp' : 'static';
      ipConfig.dnsMode = snapshot.dnsDhcpEnabled ? 'dhcp' : 'static';
      ipConfig.ipv6Mode = snapshot.ipv6DhcpEnabled !== false ? 'dhcp' : 'static';
      ipConfig.ipv6Ip = snapshot.ipv6Ip || '';
      ipConfig.ipv6Prefix = typeof snapshot.ipv6Prefix === 'number' ? snapshot.ipv6Prefix : 64;
      ipConfig.ipv6Gateway = snapshot.ipv6Gateway || '';
      ipConfig.ipv6DnsMode = snapshot.ipv6DnsDhcpEnabled !== false ? 'dhcp' : 'static';
      ipConfig.ipv6Dns1 = snapshot.ipv6Dns1 || '';
      ipConfig.ipv6Dns2 = snapshot.ipv6Dns2 || '';

      // 同步更新草稿
      adapterDrafts.set(targetAdapter, JSON.parse(JSON.stringify(ipConfig)));

      statusMsg.value = `已读取 [${targetAdapter}] 当前配置 (IPv4: ${snapshot.dhcpEnabled ? 'DHCP' : '静态'}, IPv6: ${snapshot.ipv6DhcpEnabled !== false ? 'DHCP' : '静态'})`;
      statusType.value = 'success';
    } catch (err: unknown) {
      if (reqId !== requestCounter || selectedAdapter.value !== targetAdapter) {
        return;
      }
      // 查询失败时保持表单清空，绝不残留其他网卡的历史数据 (R-04)
      clearFormFields(targetAdapter);
      statusMsg.value = `获取 [${targetAdapter}] 配置失败: ` + (err instanceof Error ? err.message : String(err));
      statusType.value = 'error';
    } finally {
      if (reqId === requestCounter) {
        isLoading.value = false;
      }
    }
  }

  // 监听适配器切换，原子处理草稿隔离与重置 (A-08, R-04)
  watch(selectedAdapter, (newAdapter, oldAdapter) => {
    if (isApplyingHistory) {
      isApplyingHistory = false;
      return;
    }

    if (oldAdapter && ipConfig.adapter === oldAdapter) {
      // 保存旧网卡的当前草稿
      adapterDrafts.set(oldAdapter, JSON.parse(JSON.stringify(ipConfig)));
    }

    if (newAdapter) {
      if (adapterDrafts.has(newAdapter)) {
        const draft = adapterDrafts.get(newAdapter)!;
        ipConfig.adapter = newAdapter;
        ipConfig.ip = draft.ip;
        ipConfig.mask = draft.mask;
        ipConfig.gateway = draft.gateway;
        ipConfig.dns1 = draft.dns1;
        ipConfig.dns2 = draft.dns2;
        ipConfig.doh1 = draft.doh1 ? { ...draft.doh1 } : undefined;
        ipConfig.doh2 = draft.doh2 ? { ...draft.doh2 } : undefined;
        ipConfig.ipv6Enabled = typeof draft.ipv6Enabled === 'boolean' ? draft.ipv6Enabled : undefined;
        ipConfig.ipMode = draft.ipMode || (currentSnapshot.value?.dhcpEnabled ? 'dhcp' : 'static');
        ipConfig.dnsMode = draft.dnsMode || (currentSnapshot.value?.dnsDhcpEnabled ? 'dhcp' : 'static');
        ipConfig.ipv6Mode = draft.ipv6Mode || (currentSnapshot.value?.ipv6DhcpEnabled !== false ? 'dhcp' : 'static');
        ipConfig.ipv6Ip = draft.ipv6Ip || '';
        ipConfig.ipv6Prefix = typeof draft.ipv6Prefix === 'number' || typeof draft.ipv6Prefix === 'string' ? draft.ipv6Prefix : 64;
        ipConfig.ipv6Gateway = draft.ipv6Gateway || '';
        ipConfig.ipv6DnsMode = draft.ipv6DnsMode || (currentSnapshot.value?.ipv6DnsDhcpEnabled !== false ? 'dhcp' : 'static');
        ipConfig.ipv6Dns1 = draft.ipv6Dns1 || '';
        ipConfig.ipv6Dns2 = draft.ipv6Dns2 || '';

        // 确保快照与当前网卡严格绑定，绝不残留旧网卡的快照 (A-08, R-04, F-06)
        if (currentSnapshot.value?.adapterName !== newAdapter) {
          currentSnapshot.value = null;
          const reqId = ++requestCounter;
          networkClient
            .getCurrentConfig(newAdapter)
            .then(snapshot => {
              if (reqId === requestCounter && selectedAdapter.value === newAdapter) {
                currentSnapshot.value = snapshot;
              }
            })
            .catch(() => {
              // 静默失败，保持 null，不影响草稿编辑
            });
        }
      } else {
        // 无草稿时先立即清空表单，杜绝读取失败时跨网卡残留 (R-04)
        clearFormFields(newAdapter);
        fetchCurrentConfig(newAdapter);
      }
    }
  });

  /**
   * 应用表单配置到系统网络
   */
  async function applyConfig(): Promise<void> {
    if (!selectedAdapter.value) {
      statusMsg.value = '请先选择一个网络适配器';
      statusType.value = 'warning';
      return;
    }

    // 目标一致性检查 (R-04)
    if (ipConfig.adapter !== selectedAdapter.value) {
      statusMsg.value = '表单网卡与当前选中网卡不一致，已重新同步，请确认后再试';
      statusType.value = 'warning';
      ipConfig.adapter = selectedAdapter.value;
      return;
    }

    // 静态 IP 必填项校验 (N-07)
    if (ipConfig.ipMode === 'static') {
      if (!ipConfig.ip.trim()) {
        statusMsg.value = '静态 IP 地址不能为空';
        statusType.value = 'warning';
        return;
      }

      if (!validateIpAddress(ipConfig.ip, false)) {
        statusMsg.value = 'IP 地址格式不正确 (必须为四段 0-255 十进制数且无前导零)';
        statusType.value = 'warning';
        return;
      }

      if (!ipConfig.mask.trim()) {
        statusMsg.value = '子网掩码不能为空';
        statusType.value = 'warning';
        return;
      }

      if (!validateSubnetMask(ipConfig.mask)) {
        statusMsg.value = '子网掩码无效 (必须为连续二进制 1，如 255.255.255.0)';
        statusType.value = 'warning';
        return;
      }

      // 可选字段校验 (A-07)
      if (ipConfig.gateway.trim()) {
        const gwCheck = validateGatewayInSubnet(ipConfig.ip, ipConfig.mask, ipConfig.gateway);
        if (!gwCheck.valid) {
          statusMsg.value = gwCheck.error || '网关配置无效';
          statusType.value = 'warning';
          return;
        }
      }
    }

    // DNS 组合与 DoH 校验 (仅在静态模式下校验，N-07)
    if (ipConfig.dnsMode === 'static') {
      // DNS 组合校验：不允许单设 DNS2 而不设 DNS1 (R-02)
      if (!ipConfig.dns1.trim() && ipConfig.dns2.trim()) {
        statusMsg.value = '若配置辅助 DNS，必须先配置首选 DNS (DNS1)';
        statusType.value = 'warning';
        return;
      }

      if (ipConfig.dns1.trim() && !validateIpAddress(ipConfig.dns1, false)) {
        statusMsg.value = '首选 DNS (DNS1) 格式不正确';
        statusType.value = 'warning';
        return;
      }

      if (ipConfig.dns2.trim() && !validateIpAddress(ipConfig.dns2, false)) {
        statusMsg.value = '辅助 DNS (DNS2) 格式不正确';
        statusType.value = 'warning';
        return;
      }

      // DoH 语义校验
      if (ipConfig.doh1 && ipConfig.doh1.mode !== 'off') {
        if (!ipConfig.dns1.trim()) {
          statusMsg.value = '启用了首选 DNS 的 DoH 加密，但未配置首选 DNS 服务器 IP';
          statusType.value = 'warning';
          return;
        }
        if (ipConfig.doh1.mode === 'manual') {
          const t = ipConfig.doh1.template.trim();
          if (!t || !t.startsWith('https://')) {
            statusMsg.value = '首选 DNS 的 DoH 模板必须是以 https:// 开头的合法 URL (如 https://doh.pub/dns-query)';
            statusType.value = 'warning';
            return;
          }
        }
      }

      if (ipConfig.doh2 && ipConfig.doh2.mode !== 'off') {
        if (!ipConfig.dns2.trim()) {
          statusMsg.value = '启用了备用 DNS 的 DoH 加密，但未配置备用 DNS 服务器 IP';
          statusType.value = 'warning';
          return;
        }
        if (ipConfig.doh2.mode === 'manual') {
          const t = ipConfig.doh2.template.trim();
          if (!t || !t.startsWith('https://')) {
            statusMsg.value = '备用 DNS 的 DoH 模板必须是以 https:// 开头的合法 URL (如 https://dns.alidns.com/dns-query)';
            statusType.value = 'warning';
            return;
          }
        }
      }
    }

    // IPv6 校验 (当 IPv6 启用或未显式禁用时)
    if (ipConfig.ipv6Enabled !== false) {
      if (ipConfig.ipv6Mode === 'static') {
        if (!ipConfig.ipv6Ip?.trim()) {
          statusMsg.value = '静态 IPv6 地址不能为空';
          statusType.value = 'warning';
          return;
        }

        if (!validateIpv6Address(ipConfig.ipv6Ip)) {
          statusMsg.value = 'IPv6 地址格式不正确 (例如 2001:db8::1 或 fe80::1)';
          statusType.value = 'warning';
          return;
        }

        if (!validateIpv6Prefix(ipConfig.ipv6Prefix)) {
          statusMsg.value = 'IPv6 子网前缀长度必须为 1 到 128 之间的整数 (例如 64)';
          statusType.value = 'warning';
          return;
        }

        if (ipConfig.ipv6Gateway?.trim()) {
          if (!validateIpv6Address(ipConfig.ipv6Gateway)) {
            statusMsg.value = 'IPv6 默认网关格式不正确';
            statusType.value = 'warning';
            return;
          }
        }
      }

      if (ipConfig.ipv6DnsMode === 'static') {
        if (!ipConfig.ipv6Dns1?.trim() && ipConfig.ipv6Dns2?.trim()) {
          statusMsg.value = '若配置辅助 IPv6 DNS，必须先配置首选 IPv6 DNS (DNS1)';
          statusType.value = 'warning';
          return;
        }

        if (ipConfig.ipv6Dns1?.trim() && !validateIpv6Address(ipConfig.ipv6Dns1)) {
          statusMsg.value = '首选 IPv6 DNS (DNS1) 格式不正确';
          statusType.value = 'warning';
          return;
        }

        if (ipConfig.ipv6Dns2?.trim() && !validateIpv6Address(ipConfig.ipv6Dns2)) {
          statusMsg.value = '辅助 IPv6 DNS (DNS2) 格式不正确';
          statusType.value = 'warning';
          return;
        }
      }
    }

    isLoading.value = true;
    statusMsg.value = '正在应用网络配置并进行读回校验...';
    statusType.value = 'info';

    try {
      const payload: Ipv4Config = {
        adapter: selectedAdapter.value,
        ip: ipConfig.ip.trim(),
        mask: ipConfig.mask.trim(),
        gateway: ipConfig.gateway.trim(),
        dns1: ipConfig.dns1.trim(),
        dns2: ipConfig.dns2.trim(),
        doh1: ipConfig.doh1 ? {
          mode: ipConfig.doh1.mode,
          template: ipConfig.doh1.template.trim(),
          allowFallback: ipConfig.doh1.allowFallback,
        } : undefined,
        doh2: ipConfig.doh2 ? {
          mode: ipConfig.doh2.mode,
          template: ipConfig.doh2.template.trim(),
          allowFallback: ipConfig.doh2.allowFallback,
        } : undefined,
        ipv6Enabled: ipConfig.ipv6Enabled,
        ipMode: ipConfig.ipMode,
        dnsMode: ipConfig.dnsMode,
        ipv6Mode: ipConfig.ipv6Mode,
        ipv6Ip: ipConfig.ipv6Ip?.trim(),
        ipv6Prefix: ipConfig.ipv6Prefix !== undefined && ipConfig.ipv6Prefix !== '' ? Number(ipConfig.ipv6Prefix) : undefined,
        ipv6Gateway: ipConfig.ipv6Gateway?.trim(),
        ipv6DnsMode: ipConfig.ipv6DnsMode,
        ipv6Dns1: ipConfig.ipv6Dns1?.trim(),
        ipv6Dns2: ipConfig.ipv6Dns2?.trim(),
      };

      const result: OperationResult = await networkClient.applyAdapterIpv4Config(payload);

      if (result.success) {
        statusMsg.value = result.message;
        statusType.value = 'success';
        if (result.snapshot) {
          currentSnapshot.value = result.snapshot;
        }
        if (onConfigApplied) {
          onConfigApplied(payload);
        }
      } else {
        // N-08: 失败或状态未知时，无论是否成功回滚，必须保留并更新实际现场快照与详细诊断；若现场快照读回失败则显式清空旧快照
        if (result.snapshot) {
          currentSnapshot.value = result.snapshot;
        } else {
          currentSnapshot.value = null;
        }
        const rollbackInfo = result.rollbackMessage ? `【恢复诊断】${result.rollbackMessage}` : '';
        if (result.rolledBack) {
          statusMsg.value = `应用失败: ${result.message}。${rollbackInfo || '已执行自动回滚。'}`;
          statusType.value = 'warning';
        } else {
          statusMsg.value = `应用失败: ${result.message}。${rollbackInfo || '回滚未完成或状态未知！'}`;
          statusType.value = 'error';
        }
      }
    } catch (err: unknown) {
      statusMsg.value = '执行网络修改时发生未捕获异常: ' + (err instanceof Error ? err.message : String(err));
      statusType.value = 'error';
    } finally {
      isLoading.value = false;
    }
  }

  /**
   * 从历史记录原子回填至当前表单，彻底规避 Vue watcher 异步竞争 (R-04)
   */
  function fillFromHistory(cfg: Partial<Ipv4Config> & { ip: string; mask: string }): void {
    // 历史目标网卡身份核验：若指定了网卡但该网卡在系统中不存在，严禁静默套用到当前选中的网卡上 (R-04, F-06)
    if (cfg.adapter && !adapters.value.some(a => a.name === cfg.adapter)) {
      statusMsg.value = `历史记录绑定的网络适配器 [${cfg.adapter}] 在当前系统中已不存在，已拒绝自动套用，请先确认目标网卡`;
      statusType.value = 'warning';
      return;
    }

    const targetAdapter = cfg.adapter || selectedAdapter.value;
    if (!targetAdapter) {
      statusMsg.value = '历史配置中未包含有效的网络适配器';
      statusType.value = 'warning';
      return;
    }

    // 保存当前活动适配器的草稿
    if (selectedAdapter.value && ipConfig.adapter === selectedAdapter.value) {
      adapterDrafts.set(selectedAdapter.value, JSON.parse(JSON.stringify(ipConfig)));
    }

    // 准备目标历史草稿 (N-10: 缺失的扩展字段严禁擅自补为 true/off，保持 undefined)
    const historyDraft: Ipv4Config = {
      adapter: targetAdapter,
      ip: cfg.ip,
      mask: cfg.mask,
      gateway: cfg.gateway || '',
      dns1: cfg.dns1 || '',
      dns2: cfg.dns2 || '',
      doh1: cfg.doh1 ? { ...cfg.doh1 } : undefined,
      doh2: cfg.doh2 ? { ...cfg.doh2 } : undefined,
      ipv6Enabled: typeof cfg.ipv6Enabled === 'boolean' ? cfg.ipv6Enabled : undefined,
      ipMode: cfg.ipMode || 'static',
      dnsMode: cfg.dnsMode || 'static',
      ipv6Mode: cfg.ipv6Mode || 'dhcp',
      ipv6Ip: cfg.ipv6Ip || '',
      ipv6Prefix: typeof cfg.ipv6Prefix === 'number' || typeof cfg.ipv6Prefix === 'string' ? cfg.ipv6Prefix : 64,
      ipv6Gateway: cfg.ipv6Gateway || '',
      ipv6DnsMode: cfg.ipv6DnsMode || 'dhcp',
      ipv6Dns1: cfg.ipv6Dns1 || '',
      ipv6Dns2: cfg.ipv6Dns2 || '',
    };
    adapterDrafts.set(targetAdapter, historyDraft);

    // 若需要切换网卡，通过标记锁定 watcher 逻辑
    if (selectedAdapter.value !== targetAdapter) {
      isApplyingHistory = true;
      selectedAdapter.value = targetAdapter;
    }

    // 原子同步当前表单
    ipConfig.adapter = targetAdapter;
    ipConfig.ip = historyDraft.ip;
    ipConfig.mask = historyDraft.mask;
    ipConfig.gateway = historyDraft.gateway;
    ipConfig.dns1 = historyDraft.dns1;
    ipConfig.dns2 = historyDraft.dns2;
    ipConfig.doh1 = historyDraft.doh1 ? { ...historyDraft.doh1 } : undefined;
    ipConfig.doh2 = historyDraft.doh2 ? { ...historyDraft.doh2 } : undefined;
    ipConfig.ipv6Enabled = historyDraft.ipv6Enabled;
    ipConfig.ipMode = historyDraft.ipMode;
    ipConfig.dnsMode = historyDraft.dnsMode;
    ipConfig.ipv6Mode = historyDraft.ipv6Mode;
    ipConfig.ipv6Ip = historyDraft.ipv6Ip;
    ipConfig.ipv6Prefix = historyDraft.ipv6Prefix;
    ipConfig.ipv6Gateway = historyDraft.ipv6Gateway;
    ipConfig.ipv6DnsMode = historyDraft.ipv6DnsMode;
    ipConfig.ipv6Dns1 = historyDraft.ipv6Dns1;
    ipConfig.ipv6Dns2 = historyDraft.ipv6Dns2;

    // N-11: 若当前快照与目标网卡不匹配，取消在途旧请求、清空旧快照并异步拉取目标网卡实际现场快照
    if (currentSnapshot.value?.adapterName !== targetAdapter) {
      currentSnapshot.value = null;
      const reqId = ++requestCounter;
      networkClient
        .getCurrentConfig(targetAdapter)
        .then(snapshot => {
          if (reqId === requestCounter && selectedAdapter.value === targetAdapter) {
            currentSnapshot.value = snapshot;
          }
        })
        .catch(() => {
          // 静默失败，保持 null，不影响表单内容
        });
    }

    statusMsg.value = `已载入历史配置到 [${targetAdapter}] 表单`;
    statusType.value = 'info';
  }

  return {
    adapters,
    selectedAdapter,
    ipConfig,
    isLoading,
    statusMsg,
    statusType,
    currentSnapshot,
    loadAdapters,
    fetchCurrentConfig,
    applyConfig,
    fillFromHistory,
  };
}
