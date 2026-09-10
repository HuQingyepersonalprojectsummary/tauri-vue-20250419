param(
    [Parameter(Mandatory = $true)][datetime]$BuiltAfter,
    [switch]$PortableOnly
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$releaseRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$releaseConfig = Get-Content -LiteralPath (Join-Path $releaseRoot 'src-tauri/tauri.conf.json') -Raw | ConvertFrom-Json
$releaseVersion = $releaseConfig.package.version
$releaseProduct = $releaseConfig.package.productName
$releaseTarget = Join-Path $releaseRoot 'src-tauri/target/release'
$releaseDir = Join-Path $releaseRoot 'releases'
$releaseFiles = @(
    @{ Source = "$releaseProduct.exe"; Name = "$releaseProduct.exe" },
    @{ Source = "$releaseProduct.exe"; Name = "Windows_Network_Config_Tool_v$releaseVersion.exe" }
)
if (-not $PortableOnly) {
    $releaseFiles += @{ Source = "bundle/msi/${releaseProduct}_${releaseVersion}_x64_zh-CN.msi"; Name = "${releaseProduct}_${releaseVersion}_x64_zh-CN.msi" }
    $releaseFiles += @{ Source = "bundle/nsis/${releaseProduct}_${releaseVersion}_x64-setup.exe"; Name = "${releaseProduct}_${releaseVersion}_x64-setup.exe" }
}
# Verify the entire source set before copying; never publish stale installer output.
foreach ($releaseFile in $releaseFiles) {
    $releaseSource = Get-Item -LiteralPath (Join-Path $releaseTarget $releaseFile.Source)
    if ($releaseSource.Length -eq 0 -or $releaseSource.LastWriteTime -lt $BuiltAfter) {
        throw "Missing or stale release artifact: $($releaseSource.FullName)"
    }
}
New-Item -ItemType Directory -Path $releaseDir -Force | Out-Null
$releaseManifest = foreach ($releaseFile in $releaseFiles) {
    $releaseDestination = Join-Path $releaseDir $releaseFile.Name
    Copy-Item -LiteralPath (Join-Path $releaseTarget $releaseFile.Source) -Destination $releaseDestination -Force
    $releaseInfo = Get-Item -LiteralPath $releaseDestination
    [ordered]@{
        file = $releaseFile.Name
        bytes = $releaseInfo.Length
        sha256 = (Get-FileHash -LiteralPath $releaseDestination -Algorithm SHA256).Hash.ToLowerInvariant()
    }
}
$releaseSourcePaths = @('package.json', 'yarn.lock', 'vite.config.ts', 'index.html', 'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock', 'src-tauri/tauri.conf.json', 'src-tauri/build.rs')
$releaseSourcePaths += @(Get-ChildItem -LiteralPath (Join-Path $releaseRoot 'src'), (Join-Path $releaseRoot 'src-tauri/src'), (Join-Path $releaseRoot 'src-tauri/icons'), (Join-Path $releaseRoot 'public') -File -Recurse | ForEach-Object { [IO.Path]::GetRelativePath($releaseRoot, $_.FullName).Replace('\', '/') })
$releaseSources = foreach ($releasePath in ($releaseSourcePaths | Sort-Object -Unique)) {
    [ordered]@{ path = $releasePath; sha256 = (Get-FileHash -LiteralPath (Join-Path $releaseRoot $releasePath) -Algorithm SHA256).Hash.ToLowerInvariant() }
}
$releaseRecord = [ordered]@{
    version = $releaseVersion
    architecture = 'x64'
    generatedAt = (Get-Date).ToString('o')
    signed = $false
    artifacts = @($releaseManifest)
    sourceFiles = @($releaseSources)
}
$releaseChecksums = ($releaseManifest | ForEach-Object { "$($_.sha256)  $($_.file)" }) -join "`n"
[IO.File]::WriteAllText((Join-Path $releaseDir 'SHA256SUMS.txt'), $releaseChecksums + "`n", [Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText((Join-Path $releaseDir 'release-manifest.json'), ($releaseRecord | ConvertTo-Json -Depth 6) + "`n", [Text.UTF8Encoding]::new($false))
$releaseManifest | Format-Table
