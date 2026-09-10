import { computed, ref, reactive, watch } from 'vue';
import type { AdapterInfo, AdapterSnapshot, Ipv4Config, OperationResult } from '../types/network';
import { networkClient } from '../services/networkClient';
import {
  validateIpAddress,
  validateSubnetMask,
  validateGatewayInSubnet,
  validateIpv6Address,
  validateIpv6Prefix,
} from '../utils/validation';

/**
 * 网络配置业务管理组合式函数 (useNetworkConfig)
 * 
 * 核心架构职责：
 * 1. 适配器生命周期管理：枚举系统所有可用网卡、选中切换与配置读回；
 * 2. 独立草稿隔离机制：为每个网卡维护独立表单草稿缓存 (adapterDrafts)，彻底杜绝跨网卡配置串写；
 * 3. 异步并发竞态防护：引入请求计数器 (requestCounter)，丢弃陈旧请求，确保界面严格展示最新选中的网卡数据；
 * 4. 全量网络语义校验：提交前在前端进行严格的 IPv4、掩码、网关、DoH 模板及 IPv6 主备 DNS 合法性检查；
 * 5. 事务性应用与回滚反馈：调用后端应用接口，实时捕获成功、失败与回滚诊断信息并同步刷新快照。
 * 
 * @param onConfigApplied 配置应用成功后的回调函数（通常用于触发历史记录保存）
 */
export function useNetworkConfig(onConfigApplied?: (cfg: Ipv4Config) => void) {
  /** 本地系统检测到的全部网络适配器列表 */
  const adapters = ref<AdapterInfo[]>([]);
  /** 当前选中的目标网络适配器名称 */
  const selectedAdapter = ref('');
  /** 全局异步加载状态指示器 (网络扫描或配置应用中) */
  const isListing = ref(false);
  const isReading = ref(false);
  const isApplying = ref(false);
  const isLoading = computed(() => isListing.value || isReading.value || isApplying.value);
  /** 底部操作状态反馈文本 */
  const statusMsg = ref('');
  /** 状态消息类型，驱动 UI 呈现不同强调色 (info | success | warning | error) */
  const statusType = ref<'info' | 'success' | 'warning' | 'error'>('info');
  /** 当前选中网卡的底层全息实时快照 (反映最新生效的系统真实网络配置) */
  const currentSnapshot = ref<AdapterSnapshot | null>(null);
  /** 当前进程是否拥有 Windows 管理员特权 */
  const isAdmin = ref(true);

  /** 当前表单绑定的网络配置响应式对象 */
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
    ipv6Enabled: undefined,
    ipMode: 'keep',
    dnsMode: 'keep',
    ipv6Mode: undefined,
    ipv6Ip: '',
    ipv6Prefix: 64,
    ipv6Gateway: '',
    ipv6DnsMode: undefined,
    ipv6Dns1: '',
    ipv6Dns2: '',
  });

  /** 每个适配器的独立草稿缓存 Map，防止在多网卡间切换时正在编辑的数据丢失或交叉串写 (A-08, R-04) */
  const adapterDrafts = new Map<string, Ipv4Config>();

  /** 标记当前是否正在受控载入历史，防止 watcher 异步调度引发草稿串写覆盖 (R-04) */
  let isApplyingHistory = false;
  /** 异步请求时序计数器，用于过滤因快速切换网卡而产生的陈旧在途异步回调 (R-04) */
  let requestCounter = 0;

  /**
   * 清空表单字段至安全默认初始状态
   * 
   * @param adapterName 目标网络适配器名称
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
    ipConfig.ipv6Enabled = undefined;
    ipConfig.ipMode = 'keep';
    ipConfig.dnsMode = 'keep';
    ipConfig.ipv6Mode = undefined;
    ipConfig.ipv6Ip = '';
    ipConfig.ipv6Prefix = 64;
    ipConfig.ipv6Gateway = '';
    ipConfig.ipv6DnsMode = undefined;
    ipConfig.ipv6Dns1 = '';
    ipConfig.ipv6Dns2 = '';
    currentSnapshot.value = null;
  }

  /**
   * 检查当前应用进程是否具有管理员特权
   * 
   * 若底层环境不支持或调用异常（如测试或 Mock 环境），默认保持为 true，
   * 实际权限最终由后端执行阶段严格把关。
   */
  async function checkAdmin(): Promise<boolean> {
    try {
      if (typeof networkClient.checkAdminPrivilege === 'function') {
        const elevated = await networkClient.checkAdminPrivilege();
        isAdmin.value = elevated;
        return elevated;
      }
    } catch {
      // 容错处理
    }
    isAdmin.value = true;
    return true;
  }

  /**
   * 加载并枚举系统中的所有网络适配器
   * 
   * 成功获取列表后，若当前未选中任何网卡或原网卡已脱机，自动默认选中首个有效适配器。
   */
  async function loadAdapters(): Promise<void> {
    if (isLoading.value) return;
    isListing.value = true;
    statusMsg.value = '正在获取网络适配器列表...';
    statusType.value = 'info';

    // 检查管理员权限状态
    await checkAdmin();

    try {
      const list = await networkClient.getNetworkAdapters();
      adapters.value = list;
      if (list.length > 0) {
        if (!selectedAdapter.value || !list.some(a => a.name === selectedAdapter.value)) {
          selectedAdapter.value = list[0].name;
        }
      } else {
        ++requestCounter;
        isReading.value = false;
        selectedAdapter.value = '';
        clearFormFields('');
        statusMsg.value = '未检测到可用的网络适配器';
        statusType.value = 'warning';
      }
    } catch (err: unknown) {
      statusMsg.value = '获取适配器列表失败: ' + (err instanceof Error ? err.message : String(err));
      statusType.value = 'error';
    } finally {
      isListing.value = false;
    }
  }

  /**
   * 获取指定网卡的当前实际现场配置快照
   * 
   * 通过递增序列号校验，丢弃陈旧请求结果，防止快速切换网卡时的竞态覆盖。
   * 读取成功后同步刷新表单字段与对应网卡的草稿缓存。
   * 
   * @param targetAdapter 目标网络适配器别名
   */
  async function fetchCurrentConfig(targetAdapter: string): Promise<void> {
    if (!targetAdapter || isApplying.value) return;
    const reqId = ++requestCounter;

    isReading.value = true;
    statusMsg.value = `正在获取 [${targetAdapter}] 的网络配置...`;
    statusType.value = 'info';

    try {
      const snapshot = await networkClient.getCurrentConfig(targetAdapter);
      // 若当前用户已切换到其他网卡，丢弃陈旧请求结果，防止竞态覆盖 (R-04)
      if (reqId !== requestCounter || selectedAdapter.value !== targetAdapter) {
        return;
      }
      currentSnapshot.value = snapshot;

      // 填充表单字段
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
      ipConfig.ipMode = 'keep';
      ipConfig.dnsMode = 'keep';
      ipConfig.ipv6Mode = undefined;
      ipConfig.ipv6Ip = snapshot.ipv6Ip || '';
      ipConfig.ipv6Prefix = typeof snapshot.ipv6Prefix === 'number' ? snapshot.ipv6Prefix : 64;
      ipConfig.ipv6Gateway = snapshot.ipv6Gateway || '';
      ipConfig.ipv6DnsMode = undefined;
      ipConfig.ipv6Dns1 = snapshot.ipv6Dns1 || '';
      ipConfig.ipv6Dns2 = snapshot.ipv6Dns2 || '';

      // 同步更新独立草稿缓存
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
        isReading.value = false;
      }
    }
  }

  // 监听适配器切换，原子处理草稿隔离与重置 (A-08, R-04)
  watch(selectedAdapter, (newAdapter, oldAdapter) => {
    if (isApplyingHistory) {
      isApplyingHistory = false;
      return;
    }

    // 旧读请求不能再覆盖新网卡或保留其加载状态。
    ++requestCounter;
    isReading.value = false;

    if (oldAdapter && ipConfig.adapter === oldAdapter) {
      // 切换前保存旧网卡的当前草稿
      adapterDrafts.set(oldAdapter, JSON.parse(JSON.stringify(ipConfig)));
    }

    if (newAdapter) {
      if (adapterDrafts.has(newAdapter)) {
        // 确保快照与当前网卡严格绑定，在恢复草稿前立即清空旧网卡快照，杜绝跨网卡快照污染 (A-08, R-04, F-06)
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
        ipConfig.ipMode = draft.ipMode || 'static';
        ipConfig.dnsMode = draft.dnsMode || 'static';
        // 保持草稿中未指定/保持语义，严禁将 undefined 强制篡改为 dhcp 意图 (R6-03)
        ipConfig.ipv6Mode = draft.ipv6Mode;
        ipConfig.ipv6Ip = draft.ipv6Ip || '';
        ipConfig.ipv6Prefix = typeof draft.ipv6Prefix === 'number' || typeof draft.ipv6Prefix === 'string'
          ? draft.ipv6Prefix
          : (draft.ipv6Mode === 'static' ? 64 : undefined);
        ipConfig.ipv6Gateway = draft.ipv6Gateway || '';
        ipConfig.ipv6DnsMode = draft.ipv6DnsMode;
        ipConfig.ipv6Dns1 = draft.ipv6Dns1 || '';
        ipConfig.ipv6Dns2 = draft.ipv6Dns2 || '';
      } else {
        // 无草稿时先立即清空表单，杜绝读取失败时跨网卡残留 (R-04)
        clearFormFields(newAdapter);
        fetchCurrentConfig(newAdapter);
      }
    } else {
      clearFormFields('');
    }
  });

  /**
   * 将当前表单中的网络配置应用到操作系统中
   * 
   * 步骤：
   * 1. 基础校验：网卡一致性、静态模式必填项、无前导零 IPv4、连续掩码、同子网网关；
   * 2. DNS 组合校验：必须优先配置 DNS1 才能配置 DNS2，DoH 模板格式校验；
   * 3. IPv6 组合校验：静态 IPv6 格式、前缀范围 1..=128、默认网关、主备 IPv6 DNS 依赖关系校验；
   * 4. 构造完整 Payload 提交后端，触发事务性配置与循环读回比对；
   * 5. 依据后端返回结果展示成功反馈或自动安全回滚诊断提示。
   */
  async function applyConfig(): Promise<void> {
    if (isLoading.value) return;
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

      // 可选网关同网段可达性校验 (A-07)
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
      if (!ipConfig.dns1.trim()) {
        statusMsg.value = '手动 IPv4 DNS 必须填写首选 DNS，如不修改请选择保持现状';
        statusType.value = 'warning';
        return;
      }
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
        if (!ipConfig.ipv6Dns1?.trim()) {
          statusMsg.value = '手动 IPv6 DNS 必须填写首选 DNS (DNS1)，如不修改请选择保持现状';
          statusType.value = 'warning';
          return;
        }

        if (ipConfig.ipv6Dns1?.trim() && !validateIpv6Address(ipConfig.ipv6Dns1)) {
          statusMsg.value = '首选 IPv6 DNS (DNS1) 格式不正确';
          statusType.value = 'warning';
          return;
        }

        if (ipConfig.ipv6Dns2?.trim() && !validateIpv6Address(ipConfig.ipv6Dns2)) {
          statusMsg.value = '备用 IPv6 DNS (DNS2) 格式不正确';
          statusType.value = 'warning';
          return;
        }
      }
    }

    // 第一次 await 前锁定 UI 和提交载荷，避免目标漂移及重复排队。
    isApplying.value = true;
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
        ipv6Mode: ipConfig.ipv6Enabled === false ? undefined : ipConfig.ipv6Mode,
        ipv6Ip: ipConfig.ipv6Enabled === false ? undefined : ipConfig.ipv6Ip?.trim(),
        ipv6Prefix: ipConfig.ipv6Enabled !== false && ipConfig.ipv6Mode === 'static' && ipConfig.ipv6Prefix !== undefined && ipConfig.ipv6Prefix !== '' ? Number(ipConfig.ipv6Prefix) : undefined,
        ipv6Gateway: ipConfig.ipv6Enabled === false ? undefined : ipConfig.ipv6Gateway?.trim(),
        ipv6DnsMode: ipConfig.ipv6Enabled === false ? undefined : ipConfig.ipv6DnsMode,
        ipv6Dns1: ipConfig.ipv6Enabled === false ? undefined : ipConfig.ipv6Dns1?.trim(),
        ipv6Dns2: ipConfig.ipv6Enabled === false ? undefined : ipConfig.ipv6Dns2?.trim(),
      };

      if (!await checkAdmin()) {
        statusMsg.value = '权限不足：修改网络配置与 DNS/DoH 需要管理员权限。请退出程序，右键点击应用图标并选择【以管理员身份运行】后再试。';
        statusType.value = 'error';
        return;
      }
      const result: OperationResult = await networkClient.applyAdapterIpv4Config(payload);
      if (result.success && onConfigApplied) onConfigApplied(payload);
      // 防御直接调用方在等待期间改变选择；结果只属于提交时的网卡。
      if (selectedAdapter.value !== payload.adapter) return;

      if (result.success) {
        statusMsg.value = result.message;
        statusType.value = 'success';
        if (result.snapshot) {
          currentSnapshot.value = result.snapshot;
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
      isApplying.value = false;
    }
  }

  /**
   * 从历史记录原子回填至当前表单，彻底规避 Vue watcher 异步竞争 (R-04)
   * 
   * @param cfg 包含历史配置字段的对象
   */
  function fillFromHistory(cfg: Partial<Ipv4Config> & { ip: string; mask: string }): void {
    if (isApplying.value) return;
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
    ++requestCounter;
    isReading.value = false;

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
      ipMode: cfg.ipMode || (cfg.ip.trim() ? 'static' : 'keep'),
      dnsMode: cfg.dnsMode || (cfg.dns1?.trim() || cfg.dns2?.trim() ? 'static' : 'keep'),
      ipv6Mode: cfg.ipv6Mode || undefined,
      ipv6Ip: cfg.ipv6Ip || '',
      ipv6Prefix: typeof cfg.ipv6Prefix === 'number' || typeof cfg.ipv6Prefix === 'string' ? cfg.ipv6Prefix : undefined,
      ipv6Gateway: cfg.ipv6Gateway || '',
      ipv6DnsMode: cfg.ipv6DnsMode || undefined,
      ipv6Dns1: cfg.ipv6Dns1 || '',
      ipv6Dns2: cfg.ipv6Dns2 || '',
    };
    adapterDrafts.set(targetAdapter, historyDraft);

    // 若需要切换网卡，通过标记锁定 watcher 逻辑，防止 watcher 内部异步清理草稿
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
    isAdmin,
    checkAdmin,
    statusMsg,
    statusType,
    currentSnapshot,
    loadAdapters,
    fetchCurrentConfig,
    applyConfig,
    fillFromHistory,
  };
}
