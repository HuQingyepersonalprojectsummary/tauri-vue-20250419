# Parse only. Never invoke the generated script or any network command.
$ErrorActionPreference = 'Stop'
$auditRepo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$auditSource = Get-Content -LiteralPath (Join-Path $auditRepo 'src-tauri/src/lib.rs') -Raw
$auditMatch = [regex]::Match($auditSource, 'let check_adapter_cmd = format!\("([^"]+)", cfg.adapter\);')
if (-not $auditMatch.Success) { throw 'Original command template changed; review this audit fixture.' }
$auditPayload = "Ethernet'; Write-Output AUDIT_MARKER; #"
$auditScript = $auditMatch.Groups[1].Value.Replace('{}', $auditPayload)
$auditTokens = $null
$auditErrors = $null
$auditAst = [System.Management.Automation.Language.Parser]::ParseInput($auditScript, [ref]$auditTokens, [ref]$auditErrors)
[pscustomobject]@{
    parseErrors = $auditErrors.Count
    commands = @($auditAst.FindAll({ param($n) $n -is [System.Management.Automation.Language.CommandAst] }, $true) | ForEach-Object { $_.GetCommandName() })
    executed = $false
} | ConvertTo-Json
