// Executes the actual composables with Vue's real watcher scheduling.
// IPC/storage are mocks. No native app or system network command is invoked.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';
const require = createRequire(import.meta.url);
const vue = require('vue');
const root = fileURLToPath(new URL('../../../', import.meta.url));
const results = [];
function load(relative, client, storage, cache = new Map()) {
  const file = path.resolve(root, relative);
  if (cache.has(file)) return cache.get(file);
  const module = { exports: {} };
  cache.set(file, module.exports);
  const js = ts.transpileModule(fs.readFileSync(file, 'utf8'), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2020 },
  }).outputText;
  const scopedRequire = spec => {
    if (spec === 'vue') return vue;
    if (spec.endsWith('/services/networkClient')) return { networkClient: client };
    if (spec.startsWith('.')) return load(path.relative(root, path.resolve(path.dirname(file), spec + '.ts')), client, storage, cache);
    throw new Error('Unexpected import: ' + spec);
  };
  vm.runInNewContext(js, { module, exports: module.exports, require: scopedRequire, localStorage: storage, console, Date, Math });
  return module.exports;
}
const snapshot = (name, ip) => ({ adapterName: name, interfaceIndex: 1, interfaceGuid: name,
  status: 'Up', dhcpEnabled: false, addresses: [], gateways: [], dnsServers: [], ip, mask: '255.255.255.0', gateway: '', dns1: '', dns2: '' });
async function flush() { for (let i = 0; i < 4; i++) { await vue.nextTick(); await Promise.resolve(); } }
function instance(getCurrentConfig) {
  const submitted = [];
  const client = { getCurrentConfig, getNetworkAdapters: async () => [{ name: 'A' }, { name: 'B' }],
    applyAdapterIpv4Config: async cfg => { submitted.push({ ...cfg }); return { success: true, message: 'mock applied', rolledBack: false }; } };
  const scope = vue.effectScope();
  const state = scope.run(() => load('src/composables/useNetworkConfig.ts', client).useNetworkConfig());
  state.adapters.value = [{ name: 'A' }, { name: 'B' }];
  return { state, submitted, scope };
}

// R-04: a normal adapter switch whose query fails retains the previous IP.
{
  const { state, submitted, scope } = instance(async name => {
    if (name === 'B') throw new Error('fixture query failed');
    return snapshot(name, '192.0.2.10');
  });
  state.selectedAdapter.value = 'A'; await flush();
  state.selectedAdapter.value = 'B'; await flush();
  const beforeApply = { selected: state.selectedAdapter.value, form: { ...state.ipConfig }, loading: state.isLoading.value };
  await state.applyConfig();
  results.push({ id: 'R-04', case: 'failed adapter read retains old address and permits write', beforeApply, submitted });
  scope.stop();
}

// R-04: fillFromHistory runs before the selectedAdapter watcher.
{
  const { state, scope } = instance(async name => snapshot(name, name === 'A' ? '192.0.2.10' : '198.51.100.20'));
  state.selectedAdapter.value = 'A'; await flush();
  state.selectedAdapter.value = 'B'; await flush();
  state.selectedAdapter.value = 'A'; await flush();
  state.fillFromHistory({ adapter: 'B', ip: '203.0.113.30', mask: '255.255.255.0' });
  await flush();
  const afterHistory = { selected: state.selectedAdapter.value, ip: state.ipConfig.ip };
  state.selectedAdapter.value = 'A'; await flush();
  results.push({ id: 'R-04', case: 'history overwritten by B draft and contaminates A draft', afterHistory,
    afterReturnToA: { selected: state.selectedAdapter.value, ip: state.ipConfig.ip } });
  scope.stop();
}

// Control case: check actual scheduling instead of assuming a loading-state race.
{
  let resolveSnapshot;
  const { state, scope } = instance(() => new Promise(resolve => { resolveSnapshot = resolve; }));
  await state.loadAdapters(); await flush();
  results.push({ id: 'control', case: 'initial snapshot pending keeps isLoading true', selected: state.selectedAdapter.value, loading: state.isLoading.value });
  resolveSnapshot(snapshot('A', '192.0.2.10')); await flush(); scope.stop();
}

// R-09: optional fields and schemaVersion are not validated.
const historyBase = { adapter: 'A', ip: '192.0.2.10', mask: '255.255.255.0', gateway: '', dns1: '', dns2: '' };
for (const [name, records] of [
  ['malformed optional field erases otherwise valid history in memory', [historyBase, { ...historyBase, gateway: 42 }]],
  ['unknown future schema relabelled as version 1', [{ ...historyBase, schemaVersion: 999 }]],
  ['null items filtered and oversized list truncated', [null, ...Array.from({ length: 12 }, () => historyBase)]],
]) {
  const storage = { getItem: () => JSON.stringify(records), setItem: () => {} };
  const h = load('src/composables/useConfigHistory.ts', {}, storage).useConfigHistory();
  h.loadConfigList();
  results.push({ id: 'R-09', case: name, count: h.configList.value.length,
    schemaVersions: h.configList.value.map(v => v.schemaVersion), warning: h.storageWarning.value });
}
// A-09 closure evidence for the actual App.vue callback wiring.
{
  const storage = { getItem: () => null, setItem: () => { throw new Error('fixture quota'); } };
  const history = load('src/composables/useConfigHistory.ts', {}, storage).useConfigHistory();
  const client = { applyAdapterIpv4Config: async () => ({ success: true, message: 'mock network applied', rolledBack: false }),
    getCurrentConfig: async name => snapshot(name, '192.0.2.10') };
  const scope = vue.effectScope();
  const state = scope.run(() => load('src/composables/useNetworkConfig.ts', client).useNetworkConfig(cfg => history.saveConfig(cfg)));
  state.selectedAdapter.value = 'A'; await flush(); await state.applyConfig();
  results.push({ id: 'A-09', case: 'storage failure preserves network success', status: state.statusType.value,
    message: state.statusMsg.value, storageWarning: history.storageWarning.value });
  scope.stop();
}
console.log(JSON.stringify({ scope: 'actual Vue composables, mocked IPC/storage, observations are not passing regression assertions', results }, null, 2));
