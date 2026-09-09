$ErrorActionPreference = 'Stop'
# Only mock cmdlets run; no system network or registry query is performed.
$probeSource = Get-Content (Join-Path $PSScriptRoot '../../../src-tauri/src/platform.rs') -Raw
$probeMatch = [regex]::Match($probeSource, '(?s)const GET_SNAPSHOT_PS_SCRIPT: &str = r#"(.*?)"#;')
if (-not $probeMatch.Success) { throw 'Snapshot script boundary changed' }
function Get-NetAdapter { [pscustomobject]@{Name='fixture';InterfaceGuid='fixture-guid';InterfaceIndex=7;Status='Up'} }
function Get-NetIPAddress { [pscustomobject]@{IPAddress='192.0.2.10';PrefixLength=24} }
function Get-NetRoute { throw 'fixture network service unavailable' }
function Get-DnsClientServerAddress { [pscustomobject]@{ServerAddresses=@('192.0.2.53')} }
function Get-NetIPInterface { [pscustomobject]@{Dhcp='Disabled'} }
function Get-ItemProperty { [pscustomobject]@{NameServer='192.0.2.53'} }
$probeOriginalIn = [Console]::In
$probeOriginalInputEncoding = [Console]::InputEncoding
$probeOriginalOutputEncoding = [Console]::OutputEncoding
try {
    # Encoding setter resets Console.In: omit only the two encoding assignments in this in-memory harness.
    $probeScript = $probeMatch.Groups[1].Value -replace '(?m)^\[Console\]::(?:Input|Output)Encoding.*$', ''
    [Console]::SetIn([IO.StringReader]::new('{"target":"fixture"}'))
    $probeResult = & ([scriptblock]::Create($probeScript)) | ConvertFrom-Json
    [pscustomobject]@{case='route_query_failure';scriptSucceeded=$true;gateways=$probeResult.gateways;dnsDhcpEnabled=$probeResult.dnsDhcpEnabled} | ConvertTo-Json -Depth 5
} finally {
    [Console]::SetIn($probeOriginalIn)
    [Console]::InputEncoding = $probeOriginalInputEncoding
    [Console]::OutputEncoding = $probeOriginalOutputEncoding
}
