// Exercises the actual mutex module with a unique audit-only name, never the app lock.
import fs from 'node:fs';import os from 'node:os';import path from 'node:path';import {spawnSync} from 'node:child_process';import {fileURLToPath} from 'node:url';
const root=fileURLToPath(new URL('../../../',import.meta.url));
const source=fs.readFileSync(path.join(root,'src-tauri/src/lib.rs'),'utf8');
const a=source.indexOf('#[cfg(windows)]'),b=source.indexOf('pub mod commands');
if(a<0||b<0)throw Error('Source boundary changed');
const dir=fs.mkdtempSync(path.join(os.tmpdir(),'network-audit4-mutex-'));
const code=source.slice(a,b)+`
fn main(){
 let name=format!("NetworkAuditOnly_{}",std::process::id());
 let guard=sys_mutex::CrossProcessLock::acquire(&format!("Global\\\\{}",name),100).unwrap();
 let worker=std::thread::spawn(move || {
   let start=std::time::Instant::now();
   let second=sys_mutex::CrossProcessLock::acquire_global_or_local(&name,150);
   println!("second_lock_acquired_while_global_held={} elapsed_ms={}",second.is_ok(),start.elapsed().as_millis());
 });
 worker.join().unwrap(); drop(guard);
}
`;
const src=path.join(dir,'main.rs'),exe=path.join(dir,'mutex.exe');fs.writeFileSync(src,code);
let r=spawnSync('rustc',['--edition=2021',src,'-o',exe],{encoding:'utf8',timeout:60000});if(r.status!==0)throw Error(r.stderr);
r=spawnSync(exe,[],{encoding:'utf8',timeout:10000});if(r.status!==0)throw Error(r.stderr);console.log(r.stdout.trim());
