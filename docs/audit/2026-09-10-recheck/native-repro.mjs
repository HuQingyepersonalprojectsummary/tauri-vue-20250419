// Actual production transaction/domain functions, with process/snapshot IO replaced.
// No system network command is reachable. Generated Rust stays in a temporary directory.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
const root = fileURLToPath(new URL('../../../', import.meta.url));
const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'network-audit4-native-'));
fs.mkdirSync(path.join(dir, 'src'));
let code = fs.readFileSync(path.join(root, 'src-tauri/src/platform.rs'), 'utf8');
function replace(start, end, replacement) {
  const a = code.indexOf(start), b = code.indexOf(end, a);
  if (a < 0 || b < 0) throw Error('Source boundary changed: ' + start);
  code = code.slice(0, a) + replacement + '\n' + code.slice(b);
}
replace('pub fn run_command_with_timeout(', '// 固定 PowerShell 脚本', `
pub fn run_command_with_timeout(_program: &Path, args: &[&str], input: Option<&[u8]>, _timeout: Duration) -> Result<ProcessOutput, String> {
 MOCK.with(|cell| { let mut m = cell.borrow_mut();
   m.calls.push(serde_json::json!({"args":args,"stdin":input.map(|b|String::from_utf8_lossy(b).to_string())}));
   if (m.scenario == "new_doh_rollback" || m.scenario == "residual_global_doh") && m.calls.len() == 3 { return Err("fixture: DoH write completed but process timed out".into()); }
   if m.scenario == "dns_timeout" && m.calls.len() == 2 { return Err("fixture DNS timeout".into()); }
   Ok(ProcessOutput {success:true,stdout:String::new(),stderr:String::new()})
 })
}
`);
replace('pub fn get_adapter_snapshot(', '/// 辅助函数', `
pub fn get_adapter_snapshot(_target: &str) -> Result<AdapterSnapshot, String> {
 MOCK.with(|cell| { let mut m = cell.borrow_mut(); m.reads += 1;
   if m.scenario == "dhcp_ipv6_only" { let mut s=snapshot(false); s.dhcp_enabled=true; s.dns_dhcp_enabled=true; return Ok(s); }
   if m.scenario == "empty_static_preflight" { let mut s=snapshot(false); s.addresses.clear(); return Ok(s); }
   if m.reads == 1 { return Ok(snapshot(false)); }
   if m.scenario == "residual_global_doh" { let mut s=snapshot(false); s.doh_settings.insert("198.51.100.53".into(),DohServerSetting{template:"https://unexpected.example/dns-query".into(),allow_fallback:true,auto_upgrade:true}); return Ok(s); }
   if m.scenario == "new_doh_rollback" { return Ok(snapshot(false)); }
   let mut s = snapshot(true);
   if m.scenario == "dns_timeout" { return Ok(s); }
   s.doh1 = Some(DohConfig {mode:"manual".into(),template:"https://unexpected.example/dns-query".into(),allow_fallback:true});
   if m.scenario == "wrong_fallback_only" { s.doh1.as_mut().unwrap().template="https://expected.example/dns-query".into(); }
   Ok(s)
 })
}
`);
if (code.includes('.spawn()')) throw Error('Unexpected process launch');
code += fs.readFileSync(new URL('native-fixtures.rs', import.meta.url), 'utf8');
fs.writeFileSync(path.join(dir, 'src/platform.rs'), code);
fs.copyFileSync(path.join(root, 'src-tauri/src/domain.rs'), path.join(dir, 'src/domain.rs'));
fs.writeFileSync(path.join(dir, 'src/main.rs'), '#![allow(dead_code, unused_imports)]\nmod domain; mod platform; fn main(){platform::audit_main();}');
fs.writeFileSync(path.join(dir, 'Cargo.toml'), '[package]\nname="audit4-native"\nversion="0.0.0"\nedition="2021"\n[dependencies]\nserde={version="=1.0.219",features=["derive"]}\nserde_json="=1.0.140"\nencoding_rs="=0.8.35"\n');
fs.copyFileSync(path.join(root, 'src-tauri/Cargo.lock'), path.join(dir, 'Cargo.lock'));
const r = spawnSync('cargo', ['run','--offline','--quiet','--manifest-path',path.join(dir,'Cargo.toml')], {encoding:'utf8',timeout:120000});
if(r.status !== 0) throw Error(r.stderr || String(r.error));
console.log(r.stdout.trim());
