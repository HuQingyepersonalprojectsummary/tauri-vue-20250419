// Compile production Rust transaction/domain logic with process and snapshot IO replaced.
// No real Windows network command is reachable.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
const root = path.resolve(import.meta.dirname, '../../..');
const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'network-functional-audit-'));
fs.mkdirSync(path.join(dir, 'src'));
let code = fs.readFileSync(path.join(root, 'src-tauri/src/platform.rs'), 'utf8');
function replace(start, end, replacement) {
  const a = code.indexOf(start), b = code.indexOf(end, a);
  assert.ok(a >= 0 && b > a, 'Production source boundary: ' + start);
  code = code.slice(0, a) + replacement + '\n' + code.slice(b);
}
replace('pub fn run_command_with_timeout(', '// 固定 PowerShell 脚本', `
pub fn run_command_with_timeout(_program: &Path, args: &[&str], input: Option<&[u8]>, _timeout: Duration) -> Result<ProcessOutput, String> {
 MOCK.with(|m| {
  let mut m=m.borrow_mut();
  let payload=input.map(|b|serde_json::from_slice::<serde_json::Value>(b).unwrap());
  m.calls.push(serde_json::json!({"args":if input.is_some(){vec!["mocked-powershell"]}else{args.to_vec()},"payload":payload}));
  if m.fail_v6_dns_once && args.get(1)==Some(&"ipv6") && args.contains(&"dnsservers") && args.contains(&"source=dhcp") {
   m.fail_v6_dns_once=false;
   return Err("fixture IPv6 DNS switch failed after DoH cleanup".into());
  }
  if let Some(p)=payload { for item in p["dohList"].as_array().unwrap() {
   if item["serverIp"]=="192.0.2.53" && item["mode"]=="off" {
    m.snap.doh1=Some(DohConfig{mode:"off".into(),template:String::new(),allow_fallback:true});
    if let Some(d)=m.snap.doh_settings.get_mut("192.0.2.53") { d.auto_upgrade=false; d.allow_fallback=true; }
   }
   if item["serverIp"]=="2001:db8::53" && item["mode"]=="off" {
    m.snap.doh_settings.get_mut("2001:db8::53").unwrap().auto_upgrade=false;
   }
  }}
  Ok(ProcessOutput{success:true,stdout:String::new(),stderr:String::new()})
 })
}
`);
replace('pub fn get_adapter_snapshot(', '/// 辅助函数', `
pub fn get_adapter_snapshot(_target: &str) -> Result<AdapterSnapshot,String> { MOCK.with(|m|Ok(m.borrow().snap.clone())) }
`);
assert.ok(!code.includes('.spawn()'), 'No system process launch');
code += fs.readFileSync(new URL('native-fixtures.rs', import.meta.url), 'utf8');
fs.writeFileSync(path.join(dir, 'src/platform.rs'), code);
fs.copyFileSync(path.join(root, 'src-tauri/src/domain.rs'), path.join(dir, 'src/domain.rs'));
fs.writeFileSync(path.join(dir, 'src/main.rs'), '#![allow(dead_code,unused_imports)]\nmod domain; mod platform; fn main(){platform::audit_main();}');
fs.writeFileSync(path.join(dir, 'Cargo.toml'), '[package]\nname="functional-audit"\nversion="0.0.0"\nedition="2021"\n[dependencies]\nserde={version="=1.0.219",features=["derive"]}\nserde_json="=1.0.140"\nencoding_rs="=0.8.35"\n');
fs.copyFileSync(path.join(root, 'src-tauri/Cargo.lock'), path.join(dir, 'Cargo.lock'));
const r = spawnSync('cargo', ['run', '--offline', '--quiet', '--manifest-path', path.join(dir, 'Cargo.toml')], { encoding: 'utf8', timeout: 120000 });
assert.equal(r.status, 0, r.stderr || String(r.error));
const rows = JSON.parse(r.stdout);
assert.ok(rows.find(r => r.case === 'disable_with_hidden_static').result.Err);
assert.ok(rows.find(r => r.case === 'disable_with_hidden_static_dns').result.Err);
const keep = rows.find(r => r.case === 'keep_dns_turns_doh_off');
assert.equal(keep.result.Ok.success, true);
assert.equal(keep.result.Ok.snapshot.doh1.mode, 'off');
const restore = rows.find(r => r.case === 'rollback_uses_global_not_adapter_doh');
assert.equal(restore.calls.find(c => c.payload)?.payload.dohList[0].mode, 'manual');
assert.equal(restore.before.doh1.mode, 'auto');
const v6 = rows.find(r => r.case === 'failed_ipv6_dns_does_not_restore_doh');
assert.equal(v6.result.Ok.rolledBack, false);
const v6Doh = v6.calls.flatMap(c => c.payload?.dohList || []).filter(d => d.serverIp === '2001:db8::53');
assert.deepEqual(v6Doh.map(d => d.mode), ['off']);
assert.equal(rows.find(r => r.case === 'static_dns_blank_keeps_previous').result.Ok.snapshot.dnsDhcpEnabled, true);
fs.writeFileSync(new URL('native-results.json', import.meta.url), JSON.stringify(rows, null, 2) + '\n');
console.log(JSON.stringify(rows.map(r => ({ case: r.case, result: r.result, calls: r.calls?.length })), null, 2));
