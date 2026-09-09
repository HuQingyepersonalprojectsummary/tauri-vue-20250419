// Exercises ONLY the real process wrapper against harmless local Rust timers.
// Does not call adapter enumeration, snapshot, rollback, or apply functions.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
const root = fileURLToPath(new URL('../../../', import.meta.url));
const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'tauri-recheck-process-'));
fs.mkdirSync(path.join(dir, 'src'));
const source = fs.readFileSync(path.join(root, 'src-tauri/src/platform.rs'), 'utf8');
const prefix = source.slice(0, source.indexOf('// 固定 PowerShell 脚本'))
  .replace(/use crate::domain::\{[\s\S]*?\};/, '');
if (prefix.includes('pub fn apply_') || prefix.includes('pub fn rollback_')) throw new Error('Unexpected system-network code');
fs.writeFileSync(path.join(dir, 'src/main.rs'), '#![allow(dead_code)]\n' + prefix + `
fn main() {
  let exe = std::env::current_exe().unwrap();
  match std::env::args().nth(1).as_deref() {
    Some("sleep") => { std::thread::sleep(Duration::from_secs(2)); return; },
    Some("parent") => {
      let _child = Command::new(&exe).arg("sleep").stdin(Stdio::null()).stdout(Stdio::inherit()).stderr(Stdio::inherit()).spawn().unwrap();
      return;
    },
    _ => {}
  }
  let start = Instant::now();
  let result = run_command_with_timeout(&exe, &["parent"], None, Duration::from_millis(500));
  let (ok, error) = match result { Ok(out) => (out.success, String::new()), Err(e) => (false, e) };
  println!("{}", serde_json::json!({"id":"R-05","timeoutMs":500,"elapsedMs":start.elapsed().as_millis(),"success":ok,"error":error}));
}
`);
fs.writeFileSync(path.join(dir, 'Cargo.toml'), `[package]
name = "tauri-process-review-evidence"
version = "0.0.0"
edition = "2021"
[dependencies]
encoding_rs = "=0.8.35"
serde_json = "=1.0.140"
`);
fs.copyFileSync(path.join(root, 'src-tauri/Cargo.lock'), path.join(dir, 'Cargo.lock'));
const result = spawnSync('cargo', ['run', '--quiet', '--offline', '--manifest-path', path.join(dir, 'Cargo.toml')], {
  encoding: 'utf8', timeout: 120000, maxBuffer: 1024 * 1024,
});
if (result.status !== 0) { process.stderr.write(result.stderr || String(result.error)); process.exit(result.status || 1); }
console.log(JSON.stringify({ scope: 'actual process wrapper; harmless parent exits while descendant holds stdout/stderr for two seconds; no network operations',
  observation: JSON.parse(result.stdout) }, null, 2));
