// Audit evidence only: executes the current Vue script with mocked IPC/storage.
// Does not launch Tauri, PowerShell, netsh, or change network settings.
import fs from 'node:fs';
import vm from 'node:vm';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';
import { parse } from '@vue/compiler-sfc';

const root = new URL('../../../', import.meta.url);
const { descriptor } = parse(fs.readFileSync(new URL('src/App.vue', root), 'utf8'));
const source = descriptor.scriptSetup.content.replace(/^import .*$/gm, '');
const js = ts.transpileModule(source, {
  compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.None },
}).outputText;
function setup(storage = {}) {
  let writes = 0;
  const context = vm.createContext({
    ref: value => ({ value }), reactive: value => value, onMounted: () => {},
    console, localStorage: { getItem: () => null, setItem: () => {}, ...storage },
    invoke: async command => {
      if (command !== 'apply_adapter_ipv4_config') throw new Error('unexpected IPC');
      writes++;
      return 'IPv4 配置已成功应用';
    },
  });
  vm.runInContext(js, context);
  return { context, writes: () => writes };
}
const observations = [];
const basic = setup();
observations.push({
  id: 'A-06', observation: 'Noncontiguous mask accepted by frontend IPv4 validator',
  observed: vm.runInContext("validateIpAddress('255.0.255.0')", basic.context),
});
const malformed = setup({ getItem: () => '[null]' });
vm.runInContext('loadConfigList()', malformed.context);
let malformedError;
try { vm.runInContext('fillFromHistory(configList.value[0])', malformed.context); }
catch (error) { malformedError = error.message; }
observations.push({ id: 'A-10', observation: 'Array element schema not checked', error: malformedError });
const oversized = setup({ getItem: () => JSON.stringify(Array.from({ length: 12 }, () => ({ adapter: 'fixture' }))) });
vm.runInContext('loadConfigList()', oversized.context);
observations.push({ id: 'A-10', observation: 'History load does not enforce 10-item limit',
  observed: vm.runInContext('configList.value.length', oversized.context) });
const failedStorage = setup({ setItem: () => { throw new Error('fixture quota exceeded'); } });
vm.runInContext("selectedAdapter.value='fixture'; Object.assign(ipConfig,{ip:'192.0.2.10',mask:'255.255.255.0'})", failedStorage.context);
await vm.runInContext('applyConfig()', failedStorage.context);
observations.push({ id: 'A-09', observation: 'Successful mocked network write reported as apply failure after storage error',
  successfulMockWrites: failedStorage.writes(), status: vm.runInContext('statusMsg.value', failedStorage.context) });
const regexSource = fs.readFileSync(new URL('src-tauri/src/lib.rs', root), 'utf8').match(/Regex::new\(r"([^"]+)"\)/)[1];
observations.push({ id: 'A-06', observation: 'Cross-language leading-zero mismatch (Rust regex evaluated with JS for this ASCII-only pattern)',
  frontend: vm.runInContext("validateIpAddress('010.0.0.1')", basic.context), rustRegex: new RegExp(regexSource).test('010.0.0.1') });
console.log(JSON.stringify({ root: fileURLToPath(root), scope: 'mocked frontend observations, not native integration tests', observations }, null, 2));
