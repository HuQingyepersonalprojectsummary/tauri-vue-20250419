// Audit observations against real Vue state; only IPC/localStorage are mocked.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import ts from 'typescript';
const root = path.resolve(import.meta.dirname, '../../..');
const vue = createRequire(import.meta.url)('vue');
function load(relative, client, cache = new Map()) {
  const file = path.resolve(root, relative);
  if (cache.has(file)) return cache.get(file);
  const module = { exports: {} }; cache.set(file, module.exports);
  const js = ts.transpileModule(fs.readFileSync(file, 'utf8'), { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2020 } }).outputText;
  const require = spec => spec === 'vue' ? vue : spec.endsWith('/services/networkClient') ? { networkClient: client } : load(path.relative(root, path.resolve(path.dirname(file), spec + '.ts')), client, cache);
  vm.runInNewContext(js, { module, exports: module.exports, require, console });
  return module.exports;
}
const snapshot = name => ({ adapterName: name, interfaceGuid: name, interfaceIndex: 1, status: 'Up', dhcpEnabled: true, dnsDhcpEnabled: true, ipv6Enabled: true, ipv6DhcpEnabled: true, ipv6DnsDhcpEnabled: true, addresses: [], gateways: [], dnsServers: [], ip: '192.0.2.10', mask: '255.255.255.0', gateway: '', dns1: '', dns2: '' });
const flush = async () => { for (let i = 0; i < 6; i++) { await vue.nextTick(); await Promise.resolve(); } };
const deferred = () => { let resolve; const promise = new Promise(r => resolve = r); return { promise, resolve }; };
const rows = [];
const scopes = [];
function make(overrides = {}) {
  const sent = [];
  const client = { getNetworkAdapters: async () => [{ name: 'A' }, { name: 'B' }], getCurrentConfig: async name => snapshot(name), applyAdapterIpv4Config: async cfg => { sent.push(JSON.parse(JSON.stringify(cfg))); return { success: true, message: 'ok', snapshot: snapshot(cfg.adapter) }; }, ...overrides };
  const scope = vue.effectScope(); scopes.push(scope);
  const state = scope.run(() => load('src/composables/useNetworkConfig.ts', client).useNetworkConfig());
  return { state, client, sent };
}
{
  const { state: s, sent } = make({ getCurrentConfig: async name => ({ ...snapshot(name), dhcpEnabled: false, dnsDhcpEnabled: false, dns1: '192.0.2.53' }) });
  await s.loadAdapters(); await flush();
  s.ipConfig.ipv6Enabled = false;
  await s.applyConfig();
  assert.equal(sent[0].ipMode, 'static'); assert.equal(sent[0].dnsMode, 'static');
  rows.push({ case: 'ipv6_toggle_resubmits_unchanged_static_ipv4_dns', payload: sent[0] });
}
{
  const { state: s, sent } = make(); await s.loadAdapters(); await flush();
  Object.assign(s.ipConfig, { ipv6Mode: 'static', ipv6Ip: '', ipv6Enabled: false });
  await s.applyConfig();
  assert.equal(sent.length, 1);
  rows.push({ case: 'disabled_ipv6_submits_hidden_invalid_static', payload: sent[0] });
}
{
  const { state: s, client } = make(); await s.loadAdapters(); await flush();
  const read = deferred(); client.getCurrentConfig = () => read.promise;
  const reading = s.fetchCurrentConfig('A');
  // History buttons are disabled during reads. This exercises the composable API boundary only.
  s.fillFromHistory({ adapter: 'A', ip: '203.0.113.42', mask: '255.255.255.0', ipv6Enabled: false });
  read.resolve(snapshot('A')); await reading; await flush();
  rows.push({ case: 'same_adapter_history_inflight_api_only', ip: s.ipConfig.ip, ipv6Enabled: s.ipConfig.ipv6Enabled });
}
{
  const { state: s, client, sent } = make(); await s.loadAdapters(); await flush();
  const admin = deferred(); client.checkAdminPrivilege = () => admin.promise;
  s.ipConfig.ipv6Enabled = false;
  const applying = s.applyConfig();
  const unlockedDuringAdmin = !s.isLoading.value;
  s.selectedAdapter.value = 'B'; await flush();
  admin.resolve(true); await applying;
  assert.equal(unlockedDuringAdmin, true); assert.equal(sent[0].adapter, 'B');
  rows.push({ case: 'admin_await_changes_apply_target', unlockedDuringAdmin, clickedAdapter: 'A', payload: sent[0] });
}
{
  const { state: s, client } = make(); await s.loadAdapters(); await flush();
  client.getNetworkAdapters = async () => [];
  await s.loadAdapters(); await flush();
  assert.equal(s.selectedAdapter.value, ''); assert.equal(s.currentSnapshot.value.adapterName, 'A');
  rows.push({ case: 'no_adapters_retains_old_snapshot', selected: s.selectedAdapter.value, snapshot: s.currentSnapshot.value.adapterName, form: s.ipConfig.adapter });
}
{
  const { state: s, client } = make(); await s.loadAdapters(); await flush();
  s.selectedAdapter.value = 'B'; await flush(); s.selectedAdapter.value = 'A'; await flush();
  const read = deferred(); client.getCurrentConfig = () => read.promise;
  // The refresh lock stops ordinary UI switching; this confirms the unguarded state API edge.
  const pending = s.fetchCurrentConfig('A'); s.selectedAdapter.value = 'B'; await flush();
  read.resolve(snapshot('A')); await pending; await flush();
  rows.push({ case: 'superseded_read_loading_api_only', isLoading: s.isLoading.value });
}
scopes.forEach(s => s.stop());
fs.writeFileSync(new URL('frontend-results.json', import.meta.url), JSON.stringify(rows, null, 2) + '\n');
console.log(JSON.stringify(rows, null, 2));
