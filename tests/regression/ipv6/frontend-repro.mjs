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
response={success:false,rolledBack:false,message:'write failed',rollbackMessage:'RECOVERY_UNKNOWN: snapshot unavailable',snapshot:null};
await s.applyConfig();
results.push({case:'unknown_recovery_retains_old_snapshot',snapshot:s.currentSnapshot.value,displayed:s.statusMsg.value});
s.fillFromHistory({adapter:'B',ip:'203.0.113.10',mask:'255.255.255.0'});await flush();
results.push({case:'history_cross_adapter_snapshot',selected:s.selectedAdapter.value,formAdapter:s.ipConfig.adapter,snapshotAdapter:s.currentSnapshot.value?.adapterName,ipv6Enabled:s.ipConfig.ipv6Enabled});
await s.applyConfig();
results.push({case:'legacy_history_ipv6_intent',payload:submitted.at(-1),status:s.statusMsg.value});
const legacyModesBefore={ip:s.ipConfig.ipv6Mode,dns:s.ipConfig.ipv6DnsMode};
s.selectedAdapter.value='A';await flush();s.selectedAdapter.value='B';await flush();
await s.applyConfig();
results.push({case:'legacy_draft_round_trip',before:legacyModesBefore,after:{ip:s.ipConfig.ipv6Mode,dns:s.ipConfig.ipv6DnsMode},payload:submitted.at(-1)});
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
const memory=new Map();const memoryStorage={getItem:key=>memory.get(key)||null,setItem:(k,v)=>memory.set(k,v),removeItem:k=>memory.delete(k)};
const hist=load('src/composables/useConfigHistory.ts',null,memoryStorage).useConfigHistory();
hist.saveConfig({adapter:'A',ip:'',mask:'255.255.255.0',gateway:'',dns1:'',dns2:'',ipMode:'dhcp',ipv6Mode:'static',ipv6Ip:'2001:db8::10',ipv6Prefix:64});
const savedCount=hist.configList.value.length;hist.loadConfigList();
results.push({case:'ipv6_only_history_lost_on_reload',savedCount,reloadedCount:hist.configList.value.length,warning:hist.storageWarning.value});
// Independent ready UI state: invalid manual DNS must never reach IPC.
client.getCurrentConfig=async name=>({...snapshot(name),ipv6Enabled:true,ipv6DhcpEnabled:true,ipv6DnsDhcpEnabled:false,ipv6DnsServers:['2001:db8::53'],ipv6Dns1:'2001:db8::53'});
const emptyScope=vue.effectScope();
const empty=emptyScope.run(()=>load('src/composables/useNetworkConfig.ts',client).useNetworkConfig());
await empty.loadAdapters();await flush();
Object.assign(empty.ipConfig,{ipMode:'keep',dnsMode:'keep',ipv6Mode:'keep',ipv6DnsMode:'static',ipv6Dns1:'',ipv6Dns2:''});
const countBefore=submitted.length;const loadingBefore=empty.isLoading.value;
await empty.applyConfig();
results.push({case:'empty_static_ipv6_dns_submitted',loadingBefore,submitted:submitted.length===countBefore+1,payload:submitted.at(-1)});
for (const [name,mode,dns1,dns2,expected] of [
 ['whitespace_dns','static',' \t ','',false],
 ['secondary_only_dns','static','','2001:db8::55',false],
 ['valid_single_dns','static','2001:db8::54','',true],
 ['valid_pair_dns','static','2001:db8::54','2001:db8::55',true],
 ['keep_empty_dns','keep','','',true],
 ['dhcp_empty_dns','dhcp','','',true],
]) {
 Object.assign(empty.ipConfig,{ipv6DnsMode:mode,ipv6Dns1:dns1,ipv6Dns2:dns2});
 const count=submitted.length;await empty.applyConfig();
 results.push({case:name,submitted:submitted.length===count+1,expected,status:empty.statusMsg.value});
}
emptyScope.stop();
console.log(JSON.stringify(results,null,2));
