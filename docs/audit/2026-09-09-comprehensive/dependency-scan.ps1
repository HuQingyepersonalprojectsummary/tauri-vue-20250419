$ErrorActionPreference = 'Stop'
$auditRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$auditLock = Get-Content -LiteralPath (Join-Path $auditRoot 'src-tauri/Cargo.lock') -Raw
$auditPackages = @([regex]::Matches($auditLock, '(?ms)^\[\[package\]\]\r?\nname = "([^"]+)"\r?\nversion = "([^"]+)"\r?\nsource = "registry\+[^\"]+"') | ForEach-Object { @{package=@{name=$_.Groups[1].Value;ecosystem='crates.io'};version=$_.Groups[2].Value} })
$auditResponse = Invoke-RestMethod -Method Post -Uri 'https://api.osv.dev/v1/querybatch' -ContentType 'application/json' -Body (@{queries=$auditPackages} | ConvertTo-Json -Depth 6 -Compress) -TimeoutSec 50
if ($auditResponse.results.Count -ne $auditPackages.Count) { throw 'Incomplete OSV response' }
$auditHits = @(for ($i=0; $i -lt $auditPackages.Count; $i++) { if ($auditResponse.results[$i].vulns) { [pscustomobject]@{package=$auditPackages[$i].package.name;version=$auditPackages[$i].version;advisories=@($auditResponse.results[$i].vulns.id)} } })
@{queried=$auditPackages.Count;queriedAt=(Get-Date).ToUniversalTime().ToString('o');hits=$auditHits} | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $PSScriptRoot 'rust-osv.json') -Encoding utf8
$auditHits | Format-Table -AutoSize
