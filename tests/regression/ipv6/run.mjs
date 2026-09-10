import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
const dir = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(dir, '../../..');
const output = path.join(dir, 'output');
fs.mkdirSync(output, { recursive: true });
for (const [program, args, file] of [
  [process.execPath, [path.join(dir, 'native-repro.mjs')], 'native-results.json'],
  [process.execPath, [path.join(dir, 'native-edge-repro.mjs')], 'native-edge-results.json'],
  [process.execPath, [path.join(dir, 'frontend-repro.mjs')], 'frontend-results.json'],
  ['pwsh', ['-NoProfile', '-File', path.join(dir, 'powershell-repro.ps1')], 'powershell-results.json'],
]) {
  console.log(`Running ${path.basename(args.at(-1))}...`);
  const result = spawnSync(program, args, { cwd: root, encoding: 'utf8', timeout: 240000, maxBuffer: 16 * 1024 * 1024 });
  if (result.error || result.status !== 0) throw new Error(result.stderr || String(result.error || result.status));
  JSON.parse(result.stdout.replace(/^\uFEFF/, ''));
  fs.writeFileSync(path.join(output, file), result.stdout);
}
await import('./verify-results.mjs');
