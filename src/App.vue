<script setup lang="ts">
import { onMounted } from 'vue';
import { useNetworkConfig } from './composables/useNetworkConfig';
import { useConfigHistory } from './composables/useConfigHistory';

const { configList, storageWarning, loadConfigList, saveConfig, removeConfig } = useConfigHistory();

const {
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
} = useNetworkConfig(appliedConfig => {
  // 网络修改成功后保存历史，若本地存储失败只产生警告，不将网络标记为失败 (A-09)
  saveConfig(appliedConfig);
});

interface DnsProviderPreset {
  name: string;
  ip1: string;
  ip2: string;
  dohTemplate1: string;
  dohTemplate2: string;
}

const PRESET_PROVIDERS: DnsProviderPreset[] = [
  {
    name: '阿里公共DNS',
    ip1: '223.5.5.5',
    ip2: '223.6.6.6',
    dohTemplate1: 'https://dns.alidns.com/dns-query',
    dohTemplate2: 'https://dns.alidns.com/dns-query',
  },
  {
    name: '腾讯DNSPod',
    ip1: '119.29.29.29',
    ip2: '1.12.12.12',
    dohTemplate1: 'https://doh.pub/dns-query',
    dohTemplate2: 'https://doh.pub/dns-query',
  },
  {
    name: 'Cloudflare',
    ip1: '1.1.1.1',
    ip2: '1.0.0.1',
    dohTemplate1: 'https://cloudflare-dns.com/dns-query',
    dohTemplate2: 'https://cloudflare-dns.com/dns-query',
  },
  {
    name: 'Google',
    ip1: '8.8.8.8',
    ip2: '8.8.4.4',
    dohTemplate1: 'https://dns.google/dns-query',
    dohTemplate2: 'https://dns.google/dns-query',
  },
];

function applyDnsPreset(preset: DnsProviderPreset) {
  ipConfig.dns1 = preset.ip1;
  ipConfig.dns2 = preset.ip2;
  ipConfig.doh1 = {
    mode: 'manual',
    template: preset.dohTemplate1,
    allowFallback: true,
  };
  ipConfig.doh2 = {
    mode: 'manual',
    template: preset.dohTemplate2,
    allowFallback: true,
  };
}

onMounted(() => {
  loadAdapters();
  loadConfigList();
});
</script>

<template>
  <main class="app-container">
    <!-- 头部导航与标题 -->
    <header class="app-header">
      <div class="header-icon">🌐</div>
      <div>
        <h1>Windows 网络配置工具</h1>
        <p class="subtitle">安全可靠的原生适配器配置，支持 IPv4 / IPv6 与 DNS over HTTPS (DoH)</p>
      </div>
    </header>

    <div class="content-grid">
      <!-- 左侧：适配器与配置表单 -->
      <section class="card config-card">
        <div class="card-header">
          <h2>适配器与网络配置</h2>
          <button
            type="button"
            class="btn-icon"
            title="重新扫描适配器"
            :disabled="isLoading"
            @click="loadAdapters"
          >
            🔄 刷新列表
          </button>
        </div>

        <!-- 适配器下拉选择 (A-08: 切换时自动载入对应网卡配置) -->
        <div class="form-group">
          <label for="adapter-select">目标网络适配器</label>
          <div class="select-wrapper">
            <select
              id="adapter-select"
              v-model="selectedAdapter"
              :disabled="isLoading || adapters.length === 0"
            >
              <option v-if="adapters.length === 0" value="">
                正在检测网络适配器...
              </option>
              <option
                v-for="a in adapters"
                :key="a.name"
                :value="a.name"
              >
                {{ a.status }}
              </option>
            </select>
          </div>
        </div>

        <!-- 当前系统状态概览 (A-05) -->
        <div v-if="currentSnapshot" class="snapshot-banner">
          <div class="snapshot-item">
            <span class="label">当前模式:</span>
            <span :class="['badge', currentSnapshot.dhcpEnabled ? 'badge-dhcp' : 'badge-static']">
              {{ currentSnapshot.dhcpEnabled ? 'DHCP 自动获取' : '静态地址' }}
            </span>
          </div>
          <div class="snapshot-item">
            <span class="label">网卡状态:</span>
            <span class="value">{{ currentSnapshot.status }}</span>
          </div>
          <div class="snapshot-item">
            <span class="label">IPv6:</span>
            <span :class="['badge', currentSnapshot.ipv6Enabled ? 'badge-v6-on' : 'badge-v6-off']">
              {{ currentSnapshot.ipv6Enabled ? '已启用' : '已禁用' }}
            </span>
          </div>
          <div v-if="currentSnapshot.doh1 && currentSnapshot.doh1.mode !== 'off'" class="snapshot-item">
            <span class="label">首选 DoH:</span>
            <span class="badge badge-doh">
              {{ currentSnapshot.doh1.mode === 'manual' ? '手动模板' : '自动' }}
            </span>
          </div>
          <div v-if="currentSnapshot.doh2 && currentSnapshot.doh2.mode !== 'off'" class="snapshot-item">
            <span class="label">备用 DoH:</span>
            <span class="badge badge-doh">
              {{ currentSnapshot.doh2.mode === 'manual' ? '手动模板' : '自动' }}
            </span>
          </div>
          <div v-if="currentSnapshot.addresses.length > 1" class="snapshot-item full-width">
            <span class="label">当前全部 IP:</span>
            <span class="value">{{ currentSnapshot.addresses.map(a => `${a.ipAddress}/${a.prefixLength}`).join(', ') }}</span>
          </div>
        </div>

        <!-- 网络配置表单 (A-07: 可选字段移除 required) -->
        <form @submit.prevent="applyConfig" class="network-form">
          <div class="form-row">
            <div class="form-group">
              <label for="input-ip">IPv4 地址 <span class="required">*</span></label>
              <input
                id="input-ip"
                v-model="ipConfig.ip"
                placeholder="例如 192.168.1.100"
                required
                :disabled="isLoading"
                autocomplete="off"
              />
            </div>

            <div class="form-group">
              <label for="input-mask">子网掩码 <span class="required">*</span></label>
              <input
                id="input-mask"
                v-model="ipConfig.mask"
                placeholder="例如 255.255.255.0"
                required
                :disabled="isLoading"
                autocomplete="off"
              />
            </div>
          </div>

          <div class="form-group">
            <label for="input-gateway">默认网关 <span class="optional">(可选)</span></label>
            <input
              id="input-gateway"
              v-model="ipConfig.gateway"
              placeholder="例如 192.168.1.1 (可留空)"
              :disabled="isLoading"
              autocomplete="off"
            />
          </div>

          <!-- 常用公共 DNS / DoH 一键预设 -->
          <div class="preset-section">
            <div class="preset-label-bar">
              <span class="preset-title">⚡ 常用公共 DNS / DoH 预设:</span>
              <span class="preset-hint">一键填入地址与加密模板</span>
            </div>
            <div class="preset-chips">
              <button
                v-for="preset in PRESET_PROVIDERS"
                :key="preset.name"
                type="button"
                class="preset-chip"
                :disabled="isLoading"
                @click="applyDnsPreset(preset)"
              >
                {{ preset.name }}
              </button>
            </div>
          </div>

          <!-- 首选 DNS 与 DoH 加密配置 (匹配 Windows 11 编辑 IP 设置) -->
          <div class="dns-section-card">
            <div class="form-group">
              <label for="input-dns1">首选 DNS 服务器 <span class="optional">(可选)</span></label>
              <input
                id="input-dns1"
                v-model="ipConfig.dns1"
                placeholder="例如 223.5.5.5 或 8.8.8.8"
                :disabled="isLoading"
                autocomplete="off"
              />
            </div>

            <div v-if="ipConfig.doh1" class="doh-panel">
              <div class="form-group">
                <label for="select-doh1">DNS over HTTPS</label>
                <div class="select-wrapper">
                  <select
                    id="select-doh1"
                    v-model="ipConfig.doh1.mode"
                    :disabled="isLoading"
                  >
                    <option value="off">关</option>
                    <option value="auto">开(自动)</option>
                    <option value="manual">开(手动模板)</option>
                  </select>
                </div>
              </div>

              <div v-if="ipConfig.doh1.mode === 'manual'" class="form-group doh-template-group">
                <label for="input-doh1-template">DNS over HTTPS 模板 <span class="required">*</span></label>
                <input
                  id="input-doh1-template"
                  v-model="ipConfig.doh1.template"
                  placeholder="https://doh.pub/dns-query"
                  :disabled="isLoading"
                  autocomplete="off"
                />
              </div>

              <div v-if="ipConfig.doh1.mode !== 'off'" class="switch-row">
                <span class="switch-label-text">失败时使用未加密请求</span>
                <label class="fluent-switch">
                  <input
                    type="checkbox"
                    v-model="ipConfig.doh1.allowFallback"
                    :disabled="isLoading"
                  />
                  <span class="slider"></span>
                  <span class="switch-status">{{ ipConfig.doh1.allowFallback ? '开' : '关' }}</span>
                </label>
              </div>
            </div>
          </div>

          <!-- 备用 DNS 与 DoH 加密配置 (匹配 Windows 11 编辑 IP 设置) -->
          <div class="dns-section-card">
            <div class="form-group">
              <label for="input-dns2">备用 DNS 服务器 <span class="optional">(可选)</span></label>
              <input
                id="input-dns2"
                v-model="ipConfig.dns2"
                placeholder="例如 223.6.6.6 或 8.8.4.4"
                :disabled="isLoading"
                autocomplete="off"
              />
            </div>

            <div v-if="ipConfig.doh2" class="doh-panel">
              <div class="form-group">
                <label for="select-doh2">DNS over HTTPS</label>
                <div class="select-wrapper">
                  <select
                    id="select-doh2"
                    v-model="ipConfig.doh2.mode"
                    :disabled="isLoading"
                  >
                    <option value="off">关</option>
                    <option value="auto">开(自动)</option>
                    <option value="manual">开(手动模板)</option>
                  </select>
                </div>
              </div>

              <div v-if="ipConfig.doh2.mode === 'manual'" class="form-group doh-template-group">
                <label for="input-doh2-template">DNS over HTTPS 模板 <span class="required">*</span></label>
                <input
                  id="input-doh2-template"
                  v-model="ipConfig.doh2.template"
                  placeholder="https://dns.alidns.com/dns-query"
                  :disabled="isLoading"
                  autocomplete="off"
                />
              </div>

              <div v-if="ipConfig.doh2.mode !== 'off'" class="switch-row">
                <span class="switch-label-text">失败时使用未加密请求</span>
                <label class="fluent-switch">
                  <input
                    type="checkbox"
                    v-model="ipConfig.doh2.allowFallback"
                    :disabled="isLoading"
                  />
                  <span class="slider"></span>
                  <span class="switch-status">{{ ipConfig.doh2.allowFallback ? '开' : '关' }}</span>
                </label>
              </div>
            </div>
          </div>

          <!-- IPv6 绑定设置 (匹配 Windows 11 编辑 IP 设置) -->
          <div class="ipv6-section-card">
            <div class="ipv6-header">
              <div>
                <h3 class="ipv6-title">IPv6</h3>
                <p class="ipv6-desc">启用或禁用此适配器的 IPv6 协议组件 (ms_tcpip6)</p>
              </div>
              <label class="fluent-switch">
                <input
                  type="checkbox"
                  v-model="ipConfig.ipv6Enabled"
                  :disabled="isLoading"
                />
                <span class="slider"></span>
                <span class="switch-status">{{ ipConfig.ipv6Enabled ? '开' : '关' }}</span>
              </label>
            </div>
          </div>

          <!-- 操作按钮组 -->
          <div class="button-group">
            <button
              type="button"
              class="btn btn-secondary"
              :disabled="isLoading || !selectedAdapter"
              @click="fetchCurrentConfig(selectedAdapter)"
            >
              读回当前配置
            </button>

            <button
              type="button"
              class="btn btn-secondary"
              :disabled="isLoading || !ipConfig.ip"
              @click="saveConfig(ipConfig)"
            >
              保存至预设
            </button>

            <button
              type="submit"
              class="btn btn-primary"
              :disabled="isLoading || !selectedAdapter"
            >
              <span v-if="isLoading" class="spinner"></span>
              {{ isLoading ? '正在安全应用...' : '应用并校验配置' }}
            </button>
          </div>
        </form>

        <!-- 操作状态区 (无障碍 aria-live 支持) -->
        <div
          v-if="statusMsg"
          class="status-banner"
          :class="`status-${statusType}`"
          role="status"
          aria-live="polite"
        >
          <span class="status-icon">
            <template v-if="statusType === 'success'">✅</template>
            <template v-else-if="statusType === 'warning'">⚠️</template>
            <template v-else-if="statusType === 'error'">❌</template>
            <template v-else>ℹ️</template>
          </span>
          <span class="status-text">{{ statusMsg }}</span>
        </div>

        <!-- 本地存储独立警告 (A-09: 不干扰网络状态) -->
        <div v-if="storageWarning" class="status-banner status-warning" role="alert">
          <span class="status-icon">💾</span>
          <span class="status-text">{{ storageWarning }}</span>
        </div>
      </section>

      <!-- 右侧：配置历史记录与预设 (A-10) -->
      <section class="card history-card">
        <div class="card-header">
          <h2>配置历史记录</h2>
          <span class="history-count">{{ configList.length }} / 10</span>
        </div>

        <p class="history-tip">最近成功应用及保存的配置，点击可快速回填：</p>

        <div v-if="configList.length === 0" class="empty-history">
          <p>暂无配置历史记录</p>
          <span class="sub-text">成功应用或保存配置后将自动记录于此</span>
        </div>

        <ul v-else class="history-list">
          <li
            v-for="item in configList"
            :key="item.id"
            class="history-item"
          >
            <div class="history-info">
              <div class="history-top">
                <span class="history-adapter">{{ item.adapter }}</span>
                <span class="history-time">{{ new Date(item.timestamp).toLocaleTimeString() }}</span>
              </div>
              <div class="history-details">
                <span class="history-ip">IP: {{ item.ip }}</span>
                <span class="history-mask">掩码: {{ item.mask }}</span>
                <span v-if="item.gateway" class="history-gw">网关: {{ item.gateway }}</span>
                <span v-if="item.dns1" class="history-dns">DNS1: {{ item.dns1 }}</span>
                <span v-if="item.dns2" class="history-dns">DNS2: {{ item.dns2 }}</span>
                <span v-if="item.ipv6Enabled !== undefined" :class="['badge-sm', item.ipv6Enabled ? 'badge-v6-on' : 'badge-v6-off']">
                  IPv6: {{ item.ipv6Enabled ? '开' : '关' }}
                </span>
                <span v-if="item.doh1 && item.doh1.mode !== 'off'" class="badge-sm badge-doh">
                  DoH1: {{ item.doh1.mode === 'manual' ? '手动' : '自动' }}
                </span>
                <span v-if="item.doh2 && item.doh2.mode !== 'off'" class="badge-sm badge-doh">
                  DoH2: {{ item.doh2.mode === 'manual' ? '手动' : '自动' }}
                </span>
              </div>
            </div>
            <div class="history-actions">
              <button
                type="button"
                class="btn-sm btn-apply"
                title="回填至表单"
                :disabled="isLoading"
                @click="fillFromHistory(item)"
              >
                载入
              </button>
              <button
                type="button"
                class="btn-sm btn-delete"
                title="删除此项"
                @click="removeConfig(item.id)"
              >
                ✕
              </button>
            </div>
          </li>
        </ul>
      </section>
    </div>
  </main>
</template>

<style scoped>
.app-container {
  max-width: 1080px;
  margin: 0 auto;
  padding: 1.5rem 1rem;
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
  color: #1e293b;
}

/* 顶部标题区 */
.app-header {
  display: flex;
  align-items: center;
  gap: 1rem;
  margin-bottom: 1.5rem;
  padding-bottom: 1rem;
  border-bottom: 1px solid #e2e8f0;
}

.header-icon {
  font-size: 2.4rem;
  line-height: 1;
}

h1 {
  font-size: 1.5rem;
  font-weight: 700;
  color: #0f172a;
  margin: 0 0 0.25rem 0;
}

.subtitle {
  font-size: 0.875rem;
  color: #64748b;
  margin: 0;
}

/* 双栏响应式布局 */
.content-grid {
  display: grid;
  grid-template-columns: 1fr;
  gap: 1.5rem;
}

@media (min-width: 840px) {
  .content-grid {
    grid-template-columns: 1.35fr 1fr;
  }
}

/* 卡片容器 */
.card {
  background: #ffffff;
  border-radius: 12px;
  border: 1px solid #e2e8f0;
  box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.05), 0 2px 4px -2px rgba(0, 0, 0, 0.05);
  padding: 1.5rem;
  display: flex;
  flex-direction: column;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 1rem;
}

h2 {
  font-size: 1.15rem;
  font-weight: 600;
  color: #1e293b;
  margin: 0;
}

.btn-icon {
  background: transparent;
  border: 1px solid #cbd5e1;
  border-radius: 6px;
  padding: 0.35rem 0.75rem;
  font-size: 0.8rem;
  color: #475569;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-icon:hover:not(:disabled) {
  background: #f1f5f9;
  border-color: #94a3b8;
}

/* 快照横幅 */
.snapshot-banner {
  display: flex;
  flex-wrap: wrap;
  gap: 0.75rem 1.5rem;
  background: #f8fafc;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  padding: 0.75rem 1rem;
  margin-bottom: 1.25rem;
  font-size: 0.85rem;
}

.snapshot-item {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.snapshot-item.full-width {
  width: 100%;
}

.snapshot-item .label {
  color: #64748b;
}

.snapshot-item .value {
  font-weight: 500;
  color: #1e293b;
}

.badge {
  padding: 0.15rem 0.5rem;
  border-radius: 4px;
  font-size: 0.75rem;
  font-weight: 600;
}

.badge-dhcp {
  background: #dbeafe;
  color: #1e40af;
}

.badge-static {
  background: #fef3c7;
  color: #92400e;
}

.badge-v6-on {
  background: #dcfce7;
  color: #15803d;
}

.badge-v6-off {
  background: #f1f5f9;
  color: #64748b;
}

.badge-doh {
  background: #ede9fe;
  color: #6d28d9;
}

.badge-sm {
  padding: 0.1rem 0.4rem;
  border-radius: 4px;
  font-size: 0.725rem;
  font-weight: 600;
}

/* 表单结构 */
.network-form {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.form-row {
  display: grid;
  grid-template-columns: 1fr;
  gap: 1rem;
}

@media (min-width: 500px) {
  .form-row {
    grid-template-columns: 1fr 1fr;
  }
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}

label {
  font-size: 0.85rem;
  font-weight: 600;
  color: #334155;
}

.required {
  color: #ef4444;
}

.optional {
  font-size: 0.75rem;
  color: #94a3b8;
  font-weight: normal;
}

input, select {
  width: 100%;
  box-sizing: border-box;
  padding: 0.65rem 0.85rem;
  border-radius: 8px;
  border: 1px solid #cbd5e1;
  font-size: 0.925rem;
  color: #0f172a;
  background-color: #ffffff;
  transition: border-color 0.2s, box-shadow 0.2s;
}

input:focus, select:focus {
  outline: none;
  border-color: #0284c7;
  box-shadow: 0 0 0 3px rgba(2, 132, 199, 0.15);
}

input:disabled, select:disabled {
  background-color: #f1f5f9;
  color: #94a3b8;
  cursor: not-allowed;
}

/* 按钮样式 */
.button-group {
  display: flex;
  flex-wrap: wrap;
  gap: 0.75rem;
  margin-top: 0.5rem;
}

.btn {
  flex: 1;
  min-width: 120px;
  padding: 0.75rem 1rem;
  border-radius: 8px;
  font-size: 0.9rem;
  font-weight: 600;
  cursor: pointer;
  display: flex;
  justify-content: center;
  align-items: center;
  gap: 0.5rem;
  border: none;
  transition: background-color 0.2s, transform 0.1s;
}

.btn:active:not(:disabled) {
  transform: translateY(1px);
}

.btn-primary {
  background: #0284c7;
  color: #ffffff;
}

.btn-primary:hover:not(:disabled) {
  background: #0369a1;
}

.btn-secondary {
  background: #f1f5f9;
  color: #334155;
  border: 1px solid #cbd5e1;
}

.btn-secondary:hover:not(:disabled) {
  background: #e2e8f0;
}

.btn:disabled {
  background: #cbd5e1;
  color: #94a3b8;
  border-color: transparent;
  cursor: not-allowed;
}

.spinner {
  width: 14px;
  height: 14px;
  border: 2px solid #ffffff;
  border-top-color: transparent;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

/* 状态通知 */
.status-banner {
  margin-top: 1.25rem;
  padding: 0.75rem 1rem;
  border-radius: 8px;
  font-size: 0.875rem;
  display: flex;
  align-items: flex-start;
  gap: 0.6rem;
  line-height: 1.4;
}

.status-icon {
  font-size: 1rem;
  flex-shrink: 0;
}

.status-info {
  background: #f0f9ff;
  border: 1px solid #bae6fd;
  color: #0369a1;
}

.status-success {
  background: #f0fdf4;
  border: 1px solid #bbf7d0;
  color: #15803d;
}

.status-warning {
  background: #fffbeb;
  border: 1px solid #fde68a;
  color: #b45309;
}

.status-error {
  background: #fef2f2;
  border: 1px solid #fecaca;
  color: #b91c1c;
}

/* 历史卡片 */
.history-tip {
  font-size: 0.85rem;
  color: #64748b;
  margin: 0 0 0.75rem 0;
}

.history-count {
  font-size: 0.75rem;
  background: #f1f5f9;
  color: #475569;
  padding: 0.2rem 0.5rem;
  border-radius: 9999px;
  font-weight: 600;
}

.empty-history {
  padding: 2.5rem 1rem;
  text-align: center;
  color: #94a3b8;
}

.empty-history p {
  margin: 0 0 0.25rem 0;
  font-weight: 500;
}

.empty-history .sub-text {
  font-size: 0.8rem;
}

.history-list {
  list-style: none;
  padding: 0;
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
  max-height: 480px;
  overflow-y: auto;
}

.history-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0.75rem;
  background: #f8fafc;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  transition: all 0.2s;
}

.history-item:hover {
  border-color: #cbd5e1;
  background: #f1f5f9;
}

.history-info {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  overflow: hidden;
}

.history-top {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.history-adapter {
  font-weight: 600;
  font-size: 0.875rem;
  color: #1e293b;
}

.history-time {
  font-size: 0.75rem;
  color: #94a3b8;
}

.history-details {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
  font-size: 0.8rem;
  color: #475569;
}

.history-actions {
  display: flex;
  gap: 0.4rem;
}

.btn-sm {
  padding: 0.35rem 0.65rem;
  border-radius: 6px;
  font-size: 0.775rem;
  font-weight: 600;
  border: none;
  cursor: pointer;
  transition: background-color 0.2s;
}

.btn-apply {
  background: #e0f2fe;
  color: #0369a1;
}

.btn-apply:hover:not(:disabled) {
  background: #bae6fd;
}

.btn-delete {
  background: transparent;
  color: #94a3b8;
}

.btn-delete:hover {
  color: #ef4444;
  background: #fee2e2;
}

/* 常用 DNS / DoH 快捷预设条 */
.preset-section {
  background: #f8fafc;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  padding: 0.75rem 0.85rem;
}

.preset-label-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 0.5rem;
}

.preset-title {
  font-size: 0.825rem;
  font-weight: 600;
  color: #334155;
}

.preset-hint {
  font-size: 0.75rem;
  color: #94a3b8;
}

.preset-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
}

.preset-chip {
  background: #ffffff;
  border: 1px solid #cbd5e1;
  border-radius: 6px;
  padding: 0.35rem 0.65rem;
  font-size: 0.775rem;
  font-weight: 500;
  color: #334155;
  cursor: pointer;
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
}

.preset-chip:hover:not(:disabled) {
  border-color: #0284c7;
  color: #0284c7;
  background: #f0f9ff;
  transform: translateY(-1px);
}

.preset-chip:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

/* DNS 与 DoH 分组卡片 (贴合 Windows 11 设置规范) */
.dns-section-card {
  background: #f8fafc;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  padding: 0.85rem 1rem;
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.doh-panel {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  padding-top: 0.65rem;
  border-top: 1px dashed #e2e8f0;
}

.doh-template-group {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}

.switch-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0.25rem 0;
}

.switch-label-text {
  font-size: 0.85rem;
  font-weight: 500;
  color: #334155;
}

/* Windows 11 Fluent 风格 Switch 拨动开关 */
.fluent-switch {
  display: inline-flex;
  align-items: center;
  gap: 0.65rem;
  cursor: pointer;
  user-select: none;
}

.fluent-switch input {
  position: absolute;
  opacity: 0;
  width: 0;
  height: 0;
  pointer-events: none;
}

.fluent-switch .slider {
  position: relative;
  display: inline-block;
  width: 44px;
  height: 22px;
  background-color: #cbd5e1;
  border-radius: 9999px;
  transition: background-color 0.22s cubic-bezier(0.4, 0, 0.2, 1);
  box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.08);
}

.fluent-switch .slider::before {
  content: "";
  position: absolute;
  height: 16px;
  width: 16px;
  left: 3px;
  top: 3px;
  background-color: #ffffff;
  border-radius: 50%;
  transition: transform 0.22s cubic-bezier(0.4, 0, 0.2, 1);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.25);
}

/* 匹配 Windows 11 设置界面的琥珀金/高亮强调色 */
.fluent-switch input:checked + .slider {
  background-color: #f59e0b;
}

.fluent-switch input:checked + .slider::before {
  transform: translateX(22px);
}

.fluent-switch input:focus-visible + .slider {
  outline: 2px solid #0284c7;
  outline-offset: 2px;
}

.fluent-switch input:disabled + .slider {
  opacity: 0.5;
  cursor: not-allowed;
}

.fluent-switch .switch-status {
  font-size: 0.85rem;
  font-weight: 600;
  color: #334155;
  min-width: 1.5rem;
}

/* IPv6 设置卡片 */
.ipv6-section-card {
  background: #f8fafc;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  padding: 0.85rem 1rem;
}

.ipv6-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.ipv6-title {
  font-size: 0.95rem;
  font-weight: 700;
  color: #1e293b;
  margin: 0;
}

.ipv6-desc {
  font-size: 0.775rem;
  color: #64748b;
  margin: 0.2rem 0 0 0;
}

.history-dns {
  color: #0369a1;
}
</style>