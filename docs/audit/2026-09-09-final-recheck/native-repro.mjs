// Generates an isolated Cargo executable using current domain + transaction code.
// Entire process runner and snapshot reader are replaced with mocks BEFORE compilation.
// No PowerShell/netsh/network operation can be executed by this harness.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
const root = fileURLToPath(new URL('../../../', import.meta.url));
const destination = fs.mkdtempSync(path.join(os.tmpdir(), 'tauri-recheck-native-'));
fs.mkdirSync(path.join(destination, 'src'));
const original = fs.readFileSync(path.join(root, 'src-tauri/src/platform.rs'), 'utf8');
const domain = fs.readFileSync(path.join(root, 'src-tauri/src/domain.rs'), 'utf8');
function replaceSection(source, startMarker, endMarker, replacement) {
  const start = source.indexOf(startMarker), end = source.indexOf(endMarker, start);
  if (start < 0 || end < 0) throw new Error('Source boundary changed: ' + startMarker);
  return source.slice(0, start) + replacement + '\n' + source.slice(end);
}
let isolated = replaceSection(original, 'pub fn run_command_with_timeout(', '// 固定 PowerShell 脚本', `
pub fn run_command_with_timeout(_program: &Path, args: &[&str], _stdin: Option<&[u8]>, _timeout: Duration) -> Result<ProcessOutput, String> {
  MOCK.with(|cell| {
    let mut m = cell.borrow_mut();
    m.commands.push(args.iter().map(|s| s.to_string()).collect());
    let call = m.commands.len();
    if m.scenario == "direct_rollback_dns_failure" && args.get(3) == Some(&"dns") { return Err("fixture rollback DNS failure".into()); }
    if m.scenario == "dns_timeout" && call == 2 { return Err("fixture DNS timeout".into()); }
    if m.scenario == "rollback_dns_failure" {
      if call == 2 { return Ok(ProcessOutput { success: false, stdout: String::new(), stderr: "fixture apply DNS failure".into() }); }
      if call >= 4 { return Err("fixture rollback DNS failure".into()); }
    }
    Ok(ProcessOutput { success: true, stdout: String::new(), stderr: String::new() })
  })
}
`);
isolated = replaceSection(isolated, 'pub fn get_adapter_snapshot(', '/// 执行自动回滚', `
pub fn get_adapter_snapshot(_target: &str) -> Result<AdapterSnapshot, String> {
  MOCK.with(|cell| {
    let mut m = cell.borrow_mut();
    m.reads += 1;
    if m.reads == 1 { return Ok(fixture_snapshot(false)); }
    if m.scenario == "readback_failure" { return Err("fixture snapshot unavailable".into()); }
    let mut s = fixture_snapshot(true);
    if m.scenario == "dns_order_mode_identity" {
      s.gateways = vec!["192.0.2.254".into()];
      s.dns_servers = vec!["203.0.113.53".into(), "198.51.100.53".into()];
      s.dns_dhcp_enabled = true;
      s.interface_guid = "different-adapter-guid".into();
    }
    Ok(s)
  })
}
`);
isolated += fs.readFileSync(new URL('native-fixtures.rs', import.meta.url), 'utf8');
if (isolated.includes('.spawn()')) throw new Error('Unexpected process launch remains in harness');
fs.writeFileSync(path.join(destination, 'src/platform.rs'), isolated);
fs.writeFileSync(path.join(destination, 'src/domain.rs'), domain);
fs.writeFileSync(path.join(destination, 'src/main.rs'), '#![allow(dead_code, unused_imports)]\nmod domain;\nmod platform;\nfn main() { platform::audit_main(); }\n');
fs.writeFileSync(path.join(destination, 'Cargo.toml'), `[package]
name = "tauri-review-evidence"
version = "0.0.0"
edition = "2021"
[dependencies]
serde = { version = "=1.0.219", features = ["derive"] }
serde_json = "=1.0.140"
encoding_rs = "=0.8.35"
`);
fs.copyFileSync(path.join(root, 'src-tauri/Cargo.lock'), path.join(destination, 'Cargo.lock'));
const result = spawnSync('cargo', ['run', '--offline', '--quiet', '--manifest-path', path.join(destination, 'Cargo.toml')], {
  encoding: 'utf8', timeout: 120000, maxBuffer: 4 * 1024 * 1024,
});
if (result.status !== 0) {
  process.stderr.write(result.stderr || String(result.error)); process.exit(result.status || 1);
}
const hash = source => crypto.createHash('sha256').update(source).digest('hex');
console.log(JSON.stringify({ scope: 'actual Rust transaction/rollback code; process and snapshot dependencies mocked; not native integration',
  platformSha256: hash(original), domainSha256: hash(domain), observations: JSON.parse(result.stdout) }, null, 2));
