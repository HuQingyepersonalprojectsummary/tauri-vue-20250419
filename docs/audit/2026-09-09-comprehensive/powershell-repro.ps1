$ErrorActionPreference = 'Stop'
# Every network/registry command is shadowed by a local mock. No network state is read or changed.
$auditSource = Get-Content -LiteralPath (Join-Path $PSScriptRoot '../../../src-tauri/src/platform.rs') -Raw
function Extract-AuditScript($name) {
    $m = [regex]::Match($auditSource, '(?s)const ' + $name + ': &str = r#"(.*?)"#;')
    if (-not $m.Success) { throw "Source boundary changed: $name" }
    [scriptblock]::Create(($m.Groups[1].Value -replace '(?m)^\[Console\]::(?:Input|Output)Encoding.*$', ''))
}
$snapshotScript = Extract-AuditScript 'GET_SNAPSHOT_PS_SCRIPT'
$applyScript = Extract-AuditScript 'APPLY_DOH_IPV6_PS_SCRIPT'
function Get-NetAdapter { [pscustomobject]@{Name='Eth[1]';InterfaceGuid='fixture-guid';InterfaceIndex=7;Status='Up'} }
function Get-NetIPAddress { [pscustomobject]@{IPAddress='192.0.2.10';PrefixLength=24} }
function Get-NetRoute { if ($script:scenario -eq 'route_error') { throw 'fixture service unavailable' }; [pscustomobject]@{NextHop='192.0.2.1'} }
function Get-DnsClientServerAddress { [pscustomobject]@{ServerAddresses=@('192.0.2.53')} }
function Get-NetIPInterface { [pscustomobject]@{Dhcp='Disabled'} }
function Test-Path { $true }
function Get-ItemProperty { [pscustomobject]@{NameServer='192.0.2.53'} }
function Get-NetAdapterBinding { throw 'fixture IPv6 binding read failed' }
function Get-DnsClientDohServerAddress {
    if ($script:scenario -eq 'unsupported_off') { throw [System.Management.Automation.CommandNotFoundException]::new('DoH cmdlet unavailable') }
    if ($script:scenario -eq 'snapshot_errors') { throw 'fixture DoH read failed' }
    [pscustomobject]@{ServerAddress='192.0.2.53';DohTemplate='https://old.example/dns-query';AutoUpgrade=$false;AllowFallbackToUdp=$false}
}
function Enable-NetAdapterBinding { param($Name,$ComponentID,$ErrorAction) $script:bindingCalls += @(@('Eth[1]','Eth1') | Where-Object { $_ -like $Name }) }
function Disable-NetAdapterBinding { param($Name,$ComponentID,$ErrorAction) $script:bindingCalls += @(@('Eth[1]','Eth1') | Where-Object { $_ -like $Name }) }
function Set-DnsClientDohServerAddress { param($ServerAddress,$DohTemplate,$AutoUpgrade,$AllowFallbackToUdp,$ErrorAction) $script:dohCalls += @{server=$ServerAddress;template=$DohTemplate;auto=$AutoUpgrade;fallback=$AllowFallbackToUdp} }
function Add-DnsClientDohServerAddress { throw 'Unexpected mock Add call' }
function Run-AuditScript($block,$payload) {
    $oldInput = [Console]::In
    try { [Console]::SetIn([IO.StringReader]::new(($payload | ConvertTo-Json -Depth 8 -Compress))); & $block }
    finally { [Console]::SetIn($oldInput) }
}
$rows = @()
foreach ($script:scenario in @('snapshot_errors','route_error')) {
    try {
        $result = Run-AuditScript $snapshotScript @{target='Eth[1]'} | ConvertFrom-Json
        $rows += @{case=$script:scenario;success=$true;ipv6=$result.ipv6Enabled;doh=$result.dohSettings}
    } catch { $rows += @{case=$script:scenario;success=$false;error=$_.Exception.Message} }
}
$script:bindingCalls=@();$script:dohCalls=@();$script:scenario='unsupported_off'
try {
    $null=Run-AuditScript $applyScript @{adapter='Eth[1]';ipv6Enabled=$true;dohList=@(@{serverIp='192.0.2.53';mode='off';template='';allowFallback=$true})}
    $rows += @{case=$script:scenario;success=$true}
} catch { $rows += @{case=$script:scenario;success=$false;error=$_.Exception.Message;bindingMatched=$script:bindingCalls} }
$script:scenario='off_restore';$script:dohCalls=@()
$null=Run-AuditScript $applyScript @{adapter='Eth[1]';dohList=@(@{serverIp='192.0.2.53';mode='off';template='https://old.example/dns-query';allowFallback=$false})}
$rows += @{case='off_restore_forces_fallback_true';calls=$script:dohCalls}
$rows | ConvertTo-Json -Depth 10
