import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import ts from 'typescript';
import { parse, compileTemplate } from '@vue/compiler-sfc';
const root = path.resolve(import.meta.dirname, '../../..');
const vue = createRequire(import.meta.url)('vue');
function load(relative, client, storage, cache = new Map()) {
  const file = path.resolve(root, relative);
  if (cache.has(file)) return cache.get(file);
  const module = { exports: {} }; cache.set(file, module.exports);
  const js = ts.transpileModule(fs.readFileSync(file, 'utf8'), { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2020 } }).outputText;
  const require = spec => spec === 'vue' ? vue : spec.endsWith('/services/networkClient') ? { networkClient: client } : load(path.relative(root, path.resolve(path.dirname(file), spec + '.ts')), client, storage, cache);
  vm.runInNewContext(js, { module, exports: module.exports, require, localStorage: storage, console, Date, Math });
  return module.exports;
}
const snapshot = name => ({ adapterName: name, interfaceGuid: name, interfaceIndex: 1, status: 'Up', dhcpEnabled: false, dnsDhcpEnabled: false, ipv6Enabled: true, ipv6DhcpEnabled: false, ipv6DnsDhcpEnabled: false, addresses: [], gateways: [], dnsServers: ['192.0.2.53'], ip: '192.0.2.10', mask: '255.255.255.0', gateway: '', dns1: '192.0.2.53', dns2: '', ipv6Ip: '2001:db8::10', ipv6Dns1: '2001:db8::53', doh1: { mode: 'auto', template: '', allowFallback: false } });
const flush = async () => { for (let i = 0; i < 8; i++) { await vue.nextTick(); await Promise.resolve(); } };
const deferred = () => { let resolve, reject; const promise = new Promise((r, j) => { resolve = r; reject = j; }); return { promise, resolve, reject }; };
const scopes = []; const cases = [];
function make(overrides = {}) {
  const sent = []; const saved = [];
  const client = { getNetworkAdapters: async () => [{ name: 'A' }, { name: 'B' }], getCurrentConfig: async name => snapshot(name), checkAdminPrivilege: async () => true, applyAdapterIpv4Config: async cfg => { sent.push(JSON.parse(JSON.stringify(cfg))); return { success: true, message: 'ok', snapshot: snapshot(cfg.adapter) }; }, ...overrides };
  const scope = vue.effectScope(); scopes.push(scope);
  const state = scope.run(() => load('src/composables/useNetworkConfig.ts', client).useNetworkConfig(cfg => saved.push(cfg)));
  return { s: state, client, sent, saved };
}
async function ready() { const ctx = make(); await ctx.s.loadAdapters(); await flush(); return ctx; }
{
  const { s, sent } = await ready();
  assert.equal(s.ipConfig.ipMode, 'keep'); assert.equal(s.ipConfig.dnsMode, 'keep');
  assert.equal(s.ipConfig.ipv6Mode, undefined); assert.equal(s.ipConfig.ipv6DnsMode, undefined);
  s.ipConfig.ipv6Enabled = false; await s.applyConfig();
  assert.equal(sent[0].ipMode, 'keep'); assert.equal(sent[0].dnsMode, 'keep');
  cases.push('读回静态配置后只改 IPv6 不重提 IPv4/DNS');
}
{
  const { s, client, sent, saved } = await ready();
  const admin = deferred(); client.checkAdminPrivilege = () => admin.promise;
  s.ipConfig.ipv6Enabled = false;
  const pending = s.applyConfig(); assert.equal(s.isLoading.value, true);
  await s.applyConfig(); // duplicate must never reach IPC
  s.fillFromHistory({ adapter: 'B', ip: '203.0.113.1', mask: '255.255.255.0' });
  assert.equal(s.selectedAdapter.value, 'A');
  // Defensive caller mutation during await must not alter the frozen payload or label B as A.
  s.ipConfig.ipv6Enabled = true; s.selectedAdapter.value = 'B'; await flush();
  admin.resolve(true); await pending; await flush();
  assert.equal(sent.length, 1); assert.equal(sent[0].adapter, 'A'); assert.equal(sent[0].ipv6Enabled, false);
  assert.equal(saved.length, 1); assert.notEqual(s.currentSnapshot.value?.adapterName, 'A');
  assert.equal(s.isLoading.value, false);
  cases.push('权限等待锁定、载荷冻结、重复点击与跨网卡结果隔离');
}
for (const outcome of ['denied', 'throws']) {
  const { s, client, sent } = await ready();
  client.checkAdminPrivilege = async () => { if (outcome === 'throws') throw Error('unavailable'); return false; };
  await s.applyConfig(); assert.equal(s.isLoading.value, false);
  assert.equal(sent.length, outcome === 'denied' ? 0 : 1);
  cases.push(`权限检查 ${outcome} 释放忙碌状态`);
}
{
  const { s, sent } = await ready();
  Object.assign(s.ipConfig, { ipv6Enabled: false, ipv6Mode: 'static', ipv6Ip: '', ipv6Prefix: '999', ipv6DnsMode: 'static', ipv6Dns1: '' });
  await s.applyConfig(); assert.equal(sent.length, 1);
  for (const key of ['ipv6Mode', 'ipv6Ip', 'ipv6Prefix', 'ipv6DnsMode', 'ipv6Dns1']) assert.equal(sent[0][key], undefined);
  s.ipConfig.ipv6Enabled = true; await s.applyConfig(); assert.equal(sent.length, 1);
  cases.push('关闭 IPv6 忽略隐藏无效字段，重新开启仍严格校验');
}
for (const dns of ['', '  ', '999.1.1.1']) {
  const { s, sent } = await ready(); Object.assign(s.ipConfig, { dnsMode: 'static', dns1: dns, dns2: '', doh1: undefined });
  await s.applyConfig(); assert.equal(sent.length, 0); assert.equal(s.isLoading.value, false);
}
cases.push('空白或无效的手动 IPv4 DNS 在 IPC 前拒绝');
{
  const { s, client } = await ready();
  client.getNetworkAdapters = async () => []; await s.loadAdapters(); await flush();
  assert.equal(s.selectedAdapter.value, ''); assert.equal(s.currentSnapshot.value, null); assert.equal(s.ipConfig.adapter, '');
  assert.equal(s.ipConfig.ipv6Enabled, undefined); assert.equal(s.isLoading.value, false);
  cases.push('适配器消失清空旧快照及表单');
}
{
  const read = deferred(); const { s } = make({ getCurrentConfig: () => read.promise });
  await s.loadAdapters(); await flush(); assert.equal(s.isLoading.value, true);
  read.resolve(snapshot('A')); await flush(); assert.equal(s.isLoading.value, false);
  cases.push('枚举完成不会提前解除仍在读取快照的锁');
}
{
  const { s, client } = await ready(); const read = deferred(); client.getCurrentConfig = () => read.promise;
  const pending = s.fetchCurrentConfig('A');
  s.fillFromHistory({ adapter: 'A', ip: '203.0.113.42', mask: '255.255.255.0', ipv6Enabled: false });
  read.resolve(snapshot('A')); await pending; await flush();
  assert.equal(s.ipConfig.ip, '203.0.113.42'); assert.equal(s.ipConfig.ipv6Enabled, false); assert.equal(s.ipConfig.dnsMode, 'keep');
  assert.equal(s.isLoading.value, false);
  cases.push('旧读请求不覆盖同网卡历史，旧历史空 DNS 保持');
}
{
  const { s, client } = await ready(); s.selectedAdapter.value = 'B'; await flush(); s.selectedAdapter.value = 'A'; await flush();
  const read = deferred(); client.getCurrentConfig = async name => name === 'A' ? read.promise : snapshot(name);
  const pending = s.fetchCurrentConfig('A'); s.selectedAdapter.value = 'B'; await flush();
  read.resolve(snapshot('A')); await pending; await flush();
  assert.equal(s.currentSnapshot.value.adapterName, 'B'); assert.equal(s.isLoading.value, false);
  cases.push('缓存网卡切换丢弃迟到响应并清除旧读取状态');
}
{
  const memory = new Map(); const storage = { getItem: k => memory.get(k) || null, setItem: (k, v) => memory.set(k, v), removeItem: k => memory.delete(k) };
  const h = load('src/composables/useConfigHistory.ts', null, storage).useConfigHistory();
  const { s, sent } = await ready();
  h.saveConfig({ ...s.ipConfig, ipMode: 'keep', dnsMode: 'keep', ipv6Enabled: false }); h.loadConfigList();
  assert.equal(h.configList.value.length, 1);
  s.fillFromHistory(h.configList.value[0]); await s.applyConfig();
  assert.equal(sent[0].ipMode, 'keep'); assert.equal(sent[0].dnsMode, 'keep');
  memory.set('net_config_history_v1', JSON.stringify([{ adapter: 'A', ip: '192.0.2.10', mask: '255.255.255.0' }]));
  h.loadConfigList(); assert.equal(h.configList.value[0].dnsMode, 'keep'); assert.equal(h.configList.value[0].ipMode, 'static');
  cases.push('保持模式保存/重载/应用与旧历史迁移兼容');
}
const { descriptor } = parse(fs.readFileSync(path.join(root, 'src/App.vue'), 'utf8'));
const compiled = compileTemplate({ source: descriptor.template.content, filename: 'App.vue', id: 'functional' });
assert.deepEqual(compiled.errors, []);
for (const id of ['select-ip-mode', 'select-dns-mode']) {
  const select = descriptor.template.content.match(new RegExp(`<select\\s+id="${id}"[\\s\\S]*?</select>`))[0];
  assert.match(select, /value="keep"/);
}
cases.push('真实 Vue 模板编译且两个 IPv4 模式均提供保持入口');
scopes.forEach(s => s.stop());
console.log(`Passed ${cases.length} frontend functional cases:\n${cases.join('\n')}`);
