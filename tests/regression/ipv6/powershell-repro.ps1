$ErrorActionPreference='Stop'
# All network and registry commands called by production script blocks are local mocks.
$source=Get-Content -LiteralPath (Join-Path $PSScriptRoot '../../../src-tauri/src/platform.rs') -Raw
function Extract-Script($name) {
 $m=[regex]::Match($source,'(?s)const '+$name+': &str = r#"(.*?)"#;')
 if (-not $m.Success) {throw "Boundary changed: $name"}
 [scriptblock]::Create(($m.Groups[1].Value -replace '(?m)^\[Console\]::(?:Input|Output)Encoding.*$',''))
}
$snapshotScript=Extract-Script 'GET_SNAPSHOT_PS_SCRIPT'
$applyScript=Extract-Script 'APPLY_DOH_IPV6_PS_SCRIPT'
function Get-NetAdapter { [pscustomobject]@{Name='Eth[1]';InterfaceGuid='fixture-guid';InterfaceIndex=7;Status='Up'} }
function Get-NetIPAddress {
 [CmdletBinding()]param($InterfaceIndex,$AddressFamily)
 [pscustomobject]@{IPAddress= $(if($AddressFamily -eq 'IPv6'){'2001:db8::10'}else{'192.0.2.10'});PrefixLength= $(if($AddressFamily -eq 'IPv6'){64}else{24});PrefixOrigin='RouterAdvertisement';SuffixOrigin='Random'}
}
function Get-NetRoute {
 [CmdletBinding()]param($InterfaceIndex,$DestinationPrefix,$AddressFamily)
 if($AddressFamily -eq 'IPv6' -and $script:scenario -eq 'ipv6_route_error'){Write-Error 'fixture IPv6 route service error';return}
 [pscustomobject]@{NextHop= $(if($AddressFamily -eq 'IPv6'){'fe80::1'}else{'192.0.2.1'})}
}
function Get-DnsClientServerAddress {
 [CmdletBinding()]param($InterfaceIndex,$AddressFamily)
 if($AddressFamily -eq 'IPv6' -and $script:scenario -eq 'ipv6_dns_error'){Write-Error 'fixture IPv6 DNS service error';return}
 [pscustomobject]@{ServerAddresses= @($(if($AddressFamily -eq 'IPv6'){'2001:db8::53'}else{'192.0.2.53'}))}
}
function Get-NetIPInterface {
 [CmdletBinding()]param($InterfaceIndex,$AddressFamily)
 if($AddressFamily -eq 'IPv6' -and $script:scenario -eq 'ipv6_mode_error'){Write-Error 'fixture IPv6 interface service error';return}
 [pscustomobject]@{Dhcp='Disabled';RouterDiscovery='Disabled'}
}
function Test-Path {$true}
function Get-ItemProperty {[pscustomobject]@{NameServer='fixture-static'} }
function Get-NetAdapterBinding {
 [CmdletBinding()]param($ComponentID)
 if($script:scenario -eq 'binding_error'){throw 'fixture binding error'}
 [pscustomobject]@{Name='Eth[1]';Enabled=$false;ComponentID='ms_tcpip6'}
}
function Get-Command { if($script:scenario -notlike 'unsupported*'){[pscustomobject]@{Name='mock-doh'}} }
function Get-DnsClientDohServerAddress {
 [CmdletBinding()]param($ServerAddress)
 if($script:scenario -eq 'doh_error'){throw 'fixture DoH service error'}
 [pscustomobject]@{ServerAddress='192.0.2.53';DohTemplate='https://old.example/dns-query';AutoUpgrade=$false;AllowFallbackToUdp=$false}
}
# InputObject parameter set only; a stray ComponentID would now fail this mock.
function Enable-NetAdapterBinding {[CmdletBinding()]param([Parameter(Mandatory)]$InputObject) $script:bindingCalls+=@($InputObject.Name)}
function Disable-NetAdapterBinding {[CmdletBinding()]param([Parameter(Mandatory)]$InputObject) $script:bindingCalls+=@($InputObject.Name)}
function Set-DnsClientDohServerAddress {[CmdletBinding()]param($ServerAddress,$DohTemplate,$AutoUpgrade,$AllowFallbackToUdp)}
function Add-DnsClientDohServerAddress {throw 'Unexpected Add'}
function Remove-DnsClientDohServerAddress {[CmdletBinding()]param($ServerAddress) if($script:scenario -eq 'delete_error'){Write-Error 'fixture cannot delete DoH entry'}}
function Run-Script($block,$payload){
 $oldInput=[Console]::In
 try{[Console]::SetIn([IO.StringReader]::new(($payload|ConvertTo-Json -Depth 8 -Compress)));& $block}
 finally{[Console]::SetIn($oldInput)}
}
$rows=@()
foreach($script:scenario in @('normal_snapshot','binding_error','doh_error','ipv6_route_error','ipv6_dns_error','ipv6_mode_error')){
 try{$result=Run-Script $snapshotScript @{target='Eth[1]'}|ConvertFrom-Json;$rows+=@{case=$script:scenario;success=$true;snapshot=$result}}
 catch{$rows+=@{case=$script:scenario;success=$false;error=$_.Exception.Message}}
}
foreach($script:scenario in @('binding_enable','binding_unchanged','unsupported_off','unsupported_manual','delete_error')){
 $script:bindingCalls=@()
 $item=@{serverIp='192.0.2.53';mode='off';template='https://old.example/dns-query';allowFallback=$false}
 if($script:scenario -eq 'unsupported_manual'){$item.mode='manual'}
 if($script:scenario -eq 'delete_error'){$item.action='remove'}
 $payload=@{adapter='Eth[1]';ipv6Enabled=($script:scenario -ne 'binding_unchanged');dohList=@($item)}
 try{$null=Run-Script $applyScript $payload;$rows+=@{case=$script:scenario;success=$true;bindingCalls=$script:bindingCalls}}
 catch{$rows+=@{case=$script:scenario;success=$false;error=$_.Exception.Message;bindingCalls=$script:bindingCalls}}
}
$rows|ConvertTo-Json -Depth 12
