param([switch]$PortableOnly)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$releaseRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Push-Location $releaseRoot
try {
    if (-not [Environment]::Is64BitOperatingSystem -or -not $IsWindows) {
        throw 'This release script requires x64 Windows and PowerShell 7.'
    }
    if (-not (Test-Path -LiteralPath 'node_modules/@tauri-apps/cli/tauri.js')) {
        throw 'Install frontend dependencies with Yarn 1.22.22 and --frozen-lockfile first.'
    }
    $started = Get-Date
    $bundles = if ($PortableOnly) { 'none' } else { 'msi,nsis' }
    & node node_modules/@tauri-apps/cli/tauri.js build --ci --bundles $bundles -- --locked --offline
    if ($LASTEXITCODE -ne 0) { throw "Tauri build failed: $LASTEXITCODE" }
    & (Join-Path $PSScriptRoot 'export-release.ps1') -BuiltAfter $started -PortableOnly:$PortableOnly
}
finally {
    Pop-Location
}
