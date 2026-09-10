$ErrorActionPreference = 'Stop'
# Only command metadata and -WhatIf binding validation use real Windows cmdlets.
# Every command executed by the production script below is replaced by a local mock.
$rows = @()
foreach ($name in @('Enable-NetAdapterBinding','Disable-NetAdapterBinding')) {
    $metadata = Microsoft.PowerShell.Core\Get-Command $name
    $sets = @($metadata.ParameterSets | ForEach-Object { @{name=$_.Name;parameters=@($_.Parameters.Name)} })
    $fixtureObject = [Microsoft.Management.Infrastructure.CimInstance]::new('MSFT_NetAdapter')
    try {
        & $metadata -InputObject $fixtureObject -ComponentID ms_tcpip6 -WhatIf -ErrorAction Stop
        $rows += @{case='real_parameter_binding';command=$name;success=$true;sets=$sets}
    } catch {
        $rows += @{case='real_parameter_binding';command=$name;success=$false;errorId=$_.FullyQualifiedErrorId;error=$_.Exception.Message;sets=$sets}
    }
    $bindingObject = [Microsoft.Management.Infrastructure.CimInstance]::new('MSFT_NetAdapterBindingSettingData')
    try {
        & $metadata -InputObject $bindingObject -ComponentID ms_tcpip6 -WhatIf -ErrorAction Stop
        $rows += @{case='real_parameter_set_conflict';command=$name;success=$true}
    } catch {
        $rows += @{case='real_parameter_set_conflict';command=$name;success=$false;errorId=$_.FullyQualifiedErrorId;error=$_.Exception.Message}
    }
}
$source = Get-Content -LiteralPath (Join-Path $PSScriptRoot '../../../src-tauri/src/platform.rs') -Raw
function Extract-Script($name) {
    $m = [regex]::Match($source, '(?s)const ' + $name + ': &str = r#"(.*?)"#;')
    if (-not $m.Success) { throw "Source boundary changed: $name" }
    [scriptblock]::Create(($m.Groups[1].Value -replace '(?m)^\[Console\]::(?:Input|Output)Encoding.*$', ''))
}
$snapshotScript = Extract-Script 'GET_SNAPSHOT_PS_SCRIPT'
$applyScript = Extract-Script 'APPLY_DOH_IPV6_PS_SCRIPT'
function Get-NetAdapter { [pscustomobject]@{Name='Eth[1]';InterfaceGuid='fixture-guid';InterfaceIndex=7;Status='Up'} }
function Get-NetIPAddress { [pscustomobject]@{IPAddress='192.0.2.10';PrefixLength=24} }
function Get-NetRoute { [pscustomobject]@{NextHop='192.0.2.1'} }
function Get-DnsClientServerAddress { [pscustomobject]@{ServerAddresses=@('192.0.2.53')} }
function Get-NetIPInterface { [pscustomobject]@{Dhcp='Disabled'} }
function Test-Path { $true }
function Get-ItemProperty { [pscustomobject]@{NameServer='192.0.2.53'} }
function Get-NetAdapterBinding {
    if ($script:scenario -eq 'ipv6_read_error') { throw 'fixture IPv6 read failed' }
    [pscustomobject]@{Name='Eth[1]';Enabled=$false}
}
function Get-Command {
    if ($script:scenario -notlike 'unsupported*') { [pscustomobject]@{Name='mock-doh-command'} }
}
function Get-DnsClientDohServerAddress {
    if ($script:scenario -eq 'doh_read_error') { throw 'fixture DoH read failed' }
    if ($script:scenario -like 'unsupported*') { throw 'Unsupported API must not be called' }
    [pscustomobject]@{ServerAddress='192.0.2.53';DohTemplate='https://old.example/dns-query';AutoUpgrade=$false;AllowFallbackToUdp=$false}
}
# Lenient mocks here intentionally isolate capability/order behavior from the separately proven binding error.
function Enable-NetAdapterBinding { $script:bindingCalls++ }
function Disable-NetAdapterBinding { $script:bindingCalls++ }
function Set-DnsClientDohServerAddress {
    param($ServerAddress,$DohTemplate,$AutoUpgrade,$AllowFallbackToUdp,$ErrorAction)
    $script:dohCalls += @{server=$ServerAddress;template=$DohTemplate;auto=$AutoUpgrade;fallback=$AllowFallbackToUdp}
}
function Add-DnsClientDohServerAddress { throw 'Unexpected mock Add call' }
function Remove-DnsClientDohServerAddress {
    [CmdletBinding()] param($ServerAddress)
    Write-Error 'fixture: cannot delete DoH entry'
}
function Run-Script($block,$payload) {
    $oldInput = [Console]::In
    try { [Console]::SetIn([IO.StringReader]::new(($payload | ConvertTo-Json -Depth 8 -Compress))); & $block }
    finally { [Console]::SetIn($oldInput) }
}
foreach ($script:scenario in @('ipv6_read_error','doh_read_error')) {
    try {
        $result = Run-Script $snapshotScript @{target='Eth[1]'} | ConvertFrom-Json
        $rows += @{case=$script:scenario;success=$true;ipv6=$result.ipv6Enabled;doh=$result.dohSettings}
    } catch { $rows += @{case=$script:scenario;success=$false;error=$_.Exception.Message} }
}
foreach ($script:scenario in @('unsupported_off','unsupported_manual','off_restore','remove_error')) {
    $script:bindingCalls=0; $script:dohCalls=@()
    $item = @{serverIp='192.0.2.53';mode='off';template='https://old.example/dns-query';allowFallback=$false}
    if ($script:scenario -eq 'unsupported_manual') { $item.mode='manual' }
    if ($script:scenario -eq 'remove_error') { $item.action='remove' }
    $payload = @{adapter='Eth[1]';dohList=@($item)}
    if ($script:scenario -like 'unsupported*') { $payload.ipv6Enabled=$true }
    try {
        $null = Run-Script $applyScript $payload
        $rows += @{case=$script:scenario;success=$true;bindingCalls=$script:bindingCalls;dohCalls=$script:dohCalls}
    } catch { $rows += @{case=$script:scenario;success=$false;error=$_.Exception.Message;bindingCalls=$script:bindingCalls} }
}
$rows | ConvertTo-Json -Depth 12
