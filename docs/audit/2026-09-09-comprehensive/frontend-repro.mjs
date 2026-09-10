// Real Vue composables; mocked IPC and localStorage only.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
import ts from 'typescript';
const root=fileURLToPath(new URL('../../../',import.meta.url));
const vue=createRequire(import.meta.url)('vue');
function load(relative,client,storage,cache=new Map()) {
 const file=path.resolve(root,relative);if(cache.has(file))return cache.get(file);
 const module={exports:{}};cache.set(file,module.exports);
 const js=ts.transpileModule(fs.readFileSync(file,'utf8'),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2020}}).outputText;
 const require=spec=>spec==='vue'?vue:spec.endsWith('/services/networkClient')?{networkClient:client}:load(path.relative(root,path.resolve(path.dirname(file),spec+'.ts')),client,storage,cache);
 vm.runInNewContext(js,{module,exports:module.exports,require,localStorage:storage,console,Date,Math});return module.exports;
}
const snapshot=name=>({adapterName:name,interfaceGuid:name,interfaceIndex:1,status:'Up',dhcpEnabled:true,dnsDhcpEnabled:true,ipv6Enabled:false,addresses:[],gateways:[],dnsServers:[],ip:'192.0.2.10',mask:'255.255.255.0',gateway:'',dns1:'192.0.2.53',dns2:''});
async function flush(){for(let i=0;i<5;i++){await vue.nextTick();await Promise.resolve();}}
const results=[];let response={success:false,rolledBack:false,message:'DNS apply failed',rollbackMessage:'RECOVERY_UNKNOWN: manually restore old IP',snapshot:snapshot('A')};
const submitted=[];
const client={getNetworkAdapters:async()=>[{name:'A'},{name:'B'}],getCurrentConfig:async name=>snapshot(name),applyAdapterIpv4Config:async cfg=>{submitted.push(JSON.parse(JSON.stringify(cfg)));return response;}};
const scope=vue.effectScope();const s=scope.run(()=>load('src/composables/useNetworkConfig.ts',client).useNetworkConfig());
await s.loadAdapters();await flush();await s.applyConfig();
results.push({case:'failed_recovery_warning_hidden',backend:response,displayed:s.statusMsg.value,type:s.statusType.value});
results.push({case:'dhcp_form_resubmission_has_no_preserve_mode',payload:submitted[0]});
s.fillFromHistory({adapter:'B',ip:'203.0.113.10',mask:'255.255.255.0'});await flush();
results.push({case:'history_cross_adapter_snapshot',selected:s.selectedAdapter.value,formAdapter:s.ipConfig.adapter,snapshotAdapter:s.currentSnapshot.value?.adapterName,ipv6Enabled:s.ipConfig.ipv6Enabled});
const before=s.ipConfig.ip;s.fillFromHistory({adapter:'deleted',ip:'203.0.113.20',mask:'255.255.255.0'});await flush();
results.push({case:'missing_history_target_rejected',unchanged:s.ipConfig.ip===before,status:s.statusMsg.value});
s.selectedAdapter.value='A';await flush();s.selectedAdapter.value='B';await flush();s.selectedAdapter.value='A';await flush();
results.push({case:'normal_cached_switch_snapshot',selected:s.selectedAdapter.value,snapshotAdapter:s.currentSnapshot.value?.adapterName});
const pendingResolves=[];
client.getCurrentConfig=async name=>name==='B'?await new Promise(resolve=>pendingResolves.push(resolve)):snapshot(name);
const pending=s.fetchCurrentConfig('B');s.selectedAdapter.value='B';await flush();s.selectedAdapter.value='A';await flush();
pendingResolves.forEach(resolve=>resolve(snapshot('B')));await pending;await flush();
results.push({case:'superseded_query_cached_switch_loading',selected:s.selectedAdapter.value,loading:s.isLoading.value});
scope.stop();
const valid={schemaVersion:1,adapter:'A',ip:'192.0.2.10',mask:'255.255.255.0'};
const storage={getItem:key=>key==='net_config_history_v1'?JSON.stringify([valid,{...valid,schemaVersion:-1},{...valid,schemaVersion:1.5},{...valid,gateway:123}]):null,setItem(){},removeItem(){}};
const h=load('src/composables/useConfigHistory.ts',null,storage).useConfigHistory();h.loadConfigList();
results.push({case:'invalid_history_items_filtered',count:h.configList.value.length,warning:h.storageWarning.value});
console.log(JSON.stringify(results,null,2));
