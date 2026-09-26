<#
.SYNOPSIS
    商业化高强度严苛发布打包脚本 (Commercial Strict Release Builder)

.DESCRIPTION
    1. 编译全套生产级产物（Rust LTO/Opt-level=z/Strip，Vite 代码混淆与剔除日志）。
    2. 调用 Themida 3.2.6.0 (x64) 执行最严格的商业级加壳防护（虚拟机 VM、反调试、防 Dump、API 隐藏、UAC 管理员清单）。
    3. 输出商业化交付包至 releases/commercial 目录，并更新完整性校验哈希清单。
#>
param(
    [string]$ThemidaPath = 'C:\BLWJ\Themida_3.2.6.0_nCh2CZZRNLW\Themida.exe',
    [string]$ProjectFile
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Push-Location $projectRoot

try {
    Write-Host "==========================================================" -ForegroundColor Cyan
    Write-Host "  Windows 网络配置工具 - 商业化严苛打包流水线" -ForegroundColor Cyan
    Write-Host "==========================================================" -ForegroundColor Cyan

    # 1. 验证环境与工具链
    if (-not (Test-Path -LiteralPath $ThemidaPath)) {
        throw "未找到 Themida 可执行程序，请检查路径: $ThemidaPath"
    }
    Write-Host "[1/5] Themida 加壳工具已就绪: $ThemidaPath" -ForegroundColor Green

    # 2. 执行标准生产构建 (Portable + MSI + NSIS)
    Write-Host "[2/5] 正在执行全套生产构建 (Vite混淆 + Rust LTO fat + MSI + NSIS)..." -ForegroundColor Yellow
    $buildStart = Get-Date
    & pwsh -NoProfile -File (Join-Path $projectRoot 'scripts\build-release.ps1')
    Write-Host "[2/5] 基础生产构建完成！" -ForegroundColor Green

    # 3. 准备商业交付目录 releases/commercial
    $commDir = Join-Path $projectRoot 'releases\commercial'
    New-Item -ItemType Directory -Path $commDir -Force | Out-Null

    $rawExe = Join-Path $projectRoot 'releases\Windows网络配置工具.exe'
    $protectedExe = Join-Path $commDir 'Windows网络配置工具_v0.1.0_x64_Portable_Commercial.exe'

    # 4. 执行 Themida 商业加壳防护
    Write-Host "[3/5] 正在调用 Themida 执行商业级加壳与代码虚拟化保护..." -ForegroundColor Yellow
    
    # 检查工程文件
    if (-not $ProjectFile) {
        $candidateProjects = @(
            (Join-Path $projectRoot 'scripts\themida.tmd'),
            (Join-Path $projectRoot 'themida.tmd'),
            (Join-Path $projectRoot 'scripts\themida.tm'),
            (Join-Path $projectRoot 'themida.tm')
        )
        foreach ($proj in $candidateProjects) {
            if (Test-Path -LiteralPath $proj) {
                $ProjectFile = $proj
                break
            }
        }
    }

    if ($ProjectFile -and (Test-Path -LiteralPath $ProjectFile)) {
        # 命令行自动加壳
        $themidaDir = Split-Path -Parent (Get-Item -LiteralPath $ThemidaPath).FullName
        $projFull = (Get-Item -LiteralPath $ProjectFile).FullName
        Write-Host "  -> 检测到工程配置文件: $projFull" -ForegroundColor Gray
        Write-Host "  -> 目标输入: $rawExe" -ForegroundColor Gray
        Write-Host "  -> 商业输出: $protectedExe" -ForegroundColor Gray

        $proc = Start-Process -FilePath $ThemidaPath -ArgumentList "/protect `"$projFull`" /inputfile `"$rawExe`" /outputfile `"$protectedExe`" /shareconsole" -WorkingDirectory $themidaDir -Wait -PassThru
        if ($proc.ExitCode -ne 0 -or -not (Test-Path -LiteralPath $protectedExe)) {
            throw "Themida 命令行加壳失败，退出代码: $($proc.ExitCode)"
        }
        Write-Host "[3/5] 商业加壳成功完成！" -ForegroundColor Green
    }
    else {
        # 提示启动图形界面进行首次严格策略配置
        Write-Host ""
        Write-Host "【提示】当前未检测到 Themida 工程文件 (.tmd / .tm)。" -ForegroundColor Yellow
        Write-Host "为确保达到【最严格、商业化】防护标准，请在即将打开的 Themida 界面中设置并保存工程：" -ForegroundColor Yellow
        Write-Host ""
        Write-Host "  【最严格商业加壳必选策略配置清单】" -ForegroundColor White
        Write-Host "  --------------------------------------------------------" -ForegroundColor Gray
        Write-Host "  1. [Application Information]" -ForegroundColor Cyan
        Write-Host "     - Input Filename  : $rawExe" -ForegroundColor Gray
        Write-Host "     - Output Filename : $protectedExe" -ForegroundColor Gray
        Write-Host "  2. [Protection Options]" -ForegroundColor Cyan
        Write-Host "     - Anti-Debugger   : 勾选 Detect Debuggers + Hide from Debuggers + Anti-Attach" -ForegroundColor Gray
        Write-Host "     - Anti-Dump       : 勾选 Erase PE Header from Memory after unpack" -ForegroundColor Gray
        Write-Host "     - Anti-Patching   : 勾选 Memory Patch Detection + File Integrity Check" -ForegroundColor Gray
        Write-Host "     - API Protection  : 勾选 Advanced API-Wrapping (IAT 混淆隐藏)" -ForegroundColor Gray
        Write-Host "     - Resource        : 保持资源区段不压缩（保留高清图标）" -ForegroundColor Gray
        Write-Host "  3. [Virtual Machine]" -ForegroundColor Cyan
        Write-Host "     - 勾选 Entry Point Virtualization" -ForegroundColor Gray
        Write-Host "     - VM 引擎推荐     : Fish (Extreme) 或 Tiger (Extreme) 多态虚拟机" -ForegroundColor Gray
        Write-Host "  4. [Extra Options / UAC]" -ForegroundColor Cyan
        Write-Host "     - Manifest 提权   : 勾选 requireAdministrator（免去用户手动右键提权）" -ForegroundColor Gray
        Write-Host "  --------------------------------------------------------" -ForegroundColor Gray
        Write-Host "  配置完成后：点击 [Save Project] 保存为 scripts\themida.tmd，随后点击 [Protect]！" -ForegroundColor Green
        Write-Host ""

        $themidaDir = Split-Path -Parent (Get-Item -LiteralPath $ThemidaPath).FullName
        Start-Process -FilePath $ThemidaPath -WorkingDirectory $themidaDir
        Write-Host "Themida 已在前台启动，请在界面中完成加壳操作。" -ForegroundColor Cyan
        return
    }

    # 5. 复制安装包并生成商业分发清单
    Write-Host "[4/5] 整合商业安装包与分发产物..." -ForegroundColor Yellow
    Copy-Item -LiteralPath (Join-Path $projectRoot 'releases\Windows网络配置工具_0.1.0_x64-setup.exe') -Destination (Join-Path $commDir 'Windows网络配置工具_0.1.0_x64-setup.exe') -Force
    Copy-Item -LiteralPath (Join-Path $projectRoot 'releases\Windows网络配置工具_0.1.0_x64_zh-CN.msi') -Destination (Join-Path $commDir 'Windows网络配置工具_0.1.0_x64_zh-CN.msi') -Force

    # 6. 计算 SHA256 与生成发布清单
    Write-Host "[5/5] 生成商业交付完整性哈希清单..." -ForegroundColor Yellow
    $commFiles = Get-ChildItem -LiteralPath $commDir -File | Where-Object { $_.Name -notmatch 'SHA256|manifest' }
    $manifestItems = foreach ($f in $commFiles) {
        $h = (Get-FileHash -LiteralPath $f.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        [ordered]@{
            file = $f.Name
            bytes = $f.Length
            sha256 = $h
        }
    }

    $shaLines = $manifestItems | ForEach-Object { "$($_.sha256)  $($_.file)" }
    [IO.File]::WriteAllText((Join-Path $commDir 'SHA256SUMS.txt'), ($shaLines -join "`n") + "`n", [Text.UTF8Encoding]::new($false))

    $commercialRecord = [ordered]@{
        productName = "Windows网络配置工具"
        version = "0.1.0"
        tier = "Commercial-Strict"
        architecture = "x64"
        protectedWith = "Oreans Themida 3.2.6.0 (x64)"
        protectionFeatures = @(
            "Anti-Debugger (Ring3/Ring0 detection, Anti-Attach, ThreadHide)",
            "Anti-Memory-Dump (Erase PE Header from memory)",
            "Code Virtualization (Multi-Engine Polymorphic VM)",
            "API-Wrapping (Import Address Table Obfuscation)",
            "Memory & Disk Anti-Patching (Integrity Verification)",
            "UAC requireAdministrator Manifest Injection",
            "Preserved Edge WebView2 Child Process Compatibility"
        )
        builtAt = (Get-Date).ToString('o')
        artifacts = @($manifestItems)
    }
    [IO.File]::WriteAllText((Join-Path $commDir 'commercial-manifest.json'), ($commercialRecord | ConvertTo-Json -Depth 6) + "`n", [Text.UTF8Encoding]::new($false))

    Write-Host ""
    Write-Host "==========================================================" -ForegroundColor Green
    Write-Host "  商业化最严格应用包发布完成！" -ForegroundColor Green
    Write-Host "  产物目录: $commDir" -ForegroundColor Green
    Write-Host "==========================================================" -ForegroundColor Green
    $manifestItems | Format-Table -AutoSize
}
finally {
    Pop-Location
}
