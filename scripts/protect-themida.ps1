<#
.SYNOPSIS
    使用 Themida 为 Windows 网络配置工具执行应用保护与加壳打包。

.DESCRIPTION
    本脚本调用位于 C:\BLWJ\Themida_3.2.6.0_nCh2CZZRNLW\Themida.exe 的加壳工具，
    支持通过 Themida 工程文件（.tmd 或 .tm）自动化命令行加壳，
    也支持启动 Themida 图形界面进行首次配置与可视化保护。

.PARAMETER ThemidaPath
    Themida.exe 的路径，默认为 C:\BLWJ\Themida_3.2.6.0_nCh2CZZRNLW\Themida.exe。

.PARAMETER InputExe
    待加壳的可执行文件路径。默认使用 releases/Windows网络配置工具.exe 或 src-tauri/target/release/Windows网络配置工具.exe。

.PARAMETER OutputExe
    加壳后的输出文件路径。默认输出为 releases/Windows网络配置工具_protected.exe。

.PARAMETER ProjectFile
    Themida 工程文件路径（.tmd 或 .tm）。如果不传，将自动搜索当前目录或 scripts 目录下的工程文件。

.PARAMETER Gui
    强制启动 Themida 图形界面模式进行可视化配置与打包。

.PARAMETER ReplaceOriginal
    加壳完成后，是否将加壳产物替换回 releases/Windows网络配置工具.exe 并同步更新 SHA256SUMS.txt。
#>
param(
    [string]$ThemidaPath = 'C:\BLWJ\Themida_3.2.6.0_nCh2CZZRNLW\Themida.exe',
    [string]$InputExe,
    [string]$OutputExe,
    [string]$ProjectFile,
    [switch]$Gui,
    [switch]$ReplaceOriginal
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Push-Location $projectRoot

try {
    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host "  Themida 应用程序保护与加壳工具" -ForegroundColor Cyan
    Write-Host "========================================" -ForegroundColor Cyan

    # 1. 验证 Themida 存在
    if (-not (Test-Path -LiteralPath $ThemidaPath)) {
        throw "未找到 Themida 可执行文件，请确认路径：$ThemidaPath"
    }
    $themidaExe = (Get-Item -LiteralPath $ThemidaPath).FullName
    $themidaDir = Split-Path -Parent $themidaExe
    Write-Host "[1/4] 检测到 Themida 工具: $themidaExe" -ForegroundColor Green

    # 2. 定位输入目标程序
    if (-not $InputExe) {
        $candidatePaths = @(
            (Join-Path $projectRoot 'releases\Windows网络配置工具.exe'),
            (Join-Path $projectRoot 'src-tauri\target\release\Windows网络配置工具.exe')
        )
        foreach ($cand in $candidatePaths) {
            if (Test-Path -LiteralPath $cand) {
                $InputExe = $cand
                break
            }
        }
    }

    if (-not $InputExe -or -not (Test-Path -LiteralPath $InputExe)) {
        Write-Warning "未找到待加壳的目标可执行文件！"
        Write-Host "正在尝试执行项目编译 (npm run release -PortableOnly)..." -ForegroundColor Yellow
        & pwsh -NoProfile -File (Join-Path $projectRoot 'scripts\build-release.ps1') -PortableOnly
        $InputExe = Join-Path $projectRoot 'releases\Windows网络配置工具.exe'
        if (-not (Test-Path -LiteralPath $InputExe)) {
            throw "构建后仍未找到目标文件: $InputExe"
        }
    }

    $inputFull = (Get-Item -LiteralPath $InputExe).FullName
    $inputLength = (Get-Item -LiteralPath $inputFull).Length
    Write-Host "[2/4] 待保护程序: $inputFull ($([Math]::Round($inputLength / 1MB, 2)) MB)" -ForegroundColor Green

    # 3. 确定输出路径
    if (-not $OutputExe) {
        $OutputExe = Join-Path (Split-Path -Parent $inputFull) "$([IO.Path]::GetFileNameWithoutExtension($inputFull))_protected.exe"
    }
    $outputFull = [IO.Path]::GetFullPath($OutputExe)
    Write-Host "[3/4] 保护输出目标: $outputFull" -ForegroundColor Green

    # 4. 检查工程配置文件 (.tmd / .tm)
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

    # 如果指定了 -Gui 或者没有工程文件，提示启动图形界面进行配置
    if ($Gui -or -not $ProjectFile) {
        Write-Host ""
        if (-not $ProjectFile) {
            Write-Host "[提示] 未检测到现有的 Themida 工程文件 (.tmd / .tm)。" -ForegroundColor Yellow
            Write-Host "Themida 命令行加壳需要先在 GUI 中保存或导出一份工程配置。" -ForegroundColor Yellow
        }
        Write-Host "准备启动 Themida 图形界面进行可视化配置与保护..." -ForegroundColor Cyan
        Write-Host "【重要配置建议（针对 Tauri / Rust / WebView2 架构）】:" -ForegroundColor White
        Write-Host "  1. [Application Information] 输入文件设置为: $inputFull" -ForegroundColor Gray
        Write-Host "  2. [Application Information] 输出文件设置为: $outputFull" -ForegroundColor Gray
        Write-Host "  3. [Protection Options] 反调试、代码虚拟化(VM)、防内存转储可按需启用。" -ForegroundColor Gray
        Write-Host "  4. [Protection Options] 请保持资源不压缩（避免破坏应用程序图标和版本信息）。" -ForegroundColor Gray
        Write-Host "  5. [Extra Options / UAC] 推荐勾选【以管理员身份运行(requireAdministrator)】" -ForegroundColor Gray
        Write-Host "     * 本工具需要修改网络适配器配置，注入管理员清单可免除用户手动右键提权！" -ForegroundColor Gray
        Write-Host "  6. 配置完成后，点击【Save Project】保存为工程文件(如 scripts\themida.tmd)，或直接点击【Protect】完成加壳。" -ForegroundColor Gray
        Write-Host ""

        $guiArgs = @()
        if ($ProjectFile -and (Test-Path -LiteralPath $ProjectFile)) {
            $guiArgs += (Get-Item -LiteralPath $ProjectFile).FullName
        }

        Write-Host "正在启动 Themida GUI..." -ForegroundColor Green
        Start-Process -FilePath $themidaExe -ArgumentList $guiArgs -WorkingDirectory $themidaDir
        Write-Host "Themida 已启动。您可以在界面中调整配置并加壳。" -ForegroundColor Cyan
        return
    }

    # 5. 命令行加壳模式
    $projFull = (Get-Item -LiteralPath $ProjectFile).FullName
    Write-Host "[4/4] 使用工程文件自动化加壳: $projFull" -ForegroundColor Green

    $protectArgs = @(
        '/protect',
        "`"$projFull`"",
        '/inputfile',
        "`"$inputFull`"",
        '/outputfile',
        "`"$outputFull`"",
        '/shareconsole'
    )

    Write-Host "执行命令: & `"$themidaExe`" $($protectArgs -join ' ')" -ForegroundColor Gray
    $proc = Start-Process -FilePath $themidaExe -ArgumentList ($protectArgs -join ' ') -WorkingDirectory $themidaDir -Wait -PassThru

    if ($proc.ExitCode -ne 0) {
        $errMsg = switch ($proc.ExitCode) {
            1 { "工程文件不存在或格式无效 (Code 1)" }
            2 { "无法打开待加壳的目标文件 (Code 2)" }
            3 { "目标文件已经被加壳保护 (Code 3)" }
            4 { "插入的 SecureEngine 宏存在错误 (Code 4)" }
            5 { "Themida 内部保护错误 (Code 5)" }
            6 { "无法将保护后的文件写入磁盘 (Code 6)" }
            7 { "打开或读取 Splash 启动图失败 (Code 7)" }
            default { "Themida 加壳异常，退出码: $($proc.ExitCode)" }
        }
        throw "Themida 加壳失败: $errMsg"
    }

    if (-not (Test-Path -LiteralPath $outputFull)) {
        throw "Themida 报告成功，但输出文件未生成: $outputFull"
    }

    $outItem = Get-Item -LiteralPath $outputFull
    Write-Host ""
    Write-Host "========================================" -ForegroundColor Green
    Write-Host "  加壳成功！" -ForegroundColor Green
    Write-Host "  输出文件: $($outItem.FullName)" -ForegroundColor Green
    Write-Host "  文件大小: $([Math]::Round($outItem.Length / 1MB, 2)) MB" -ForegroundColor Green
    Write-Host "  SHA-256 : $((Get-FileHash -LiteralPath $outputFull -Algorithm SHA256).Hash.ToLowerInvariant())" -ForegroundColor Green
    Write-Host "========================================" -ForegroundColor Green

    # 6. 如果要求替换原文件
    if ($ReplaceOriginal) {
        Write-Host "正在将加壳文件替换原便携程序: $inputFull ..." -ForegroundColor Yellow
        Copy-Item -LiteralPath $outputFull -Destination $inputFull -Force
        
        # 同步更新 releases 目录下的 SHA256 清单与 manifest
        $releasesDir = Join-Path $projectRoot 'releases'
        if (Test-Path -LiteralPath $releasesDir) {
            $manifestPath = Join-Path $releasesDir 'release-manifest.json'
            if (Test-Path -LiteralPath $manifestPath) {
                Write-Host "正在重新生成 release 清单和哈希值..." -ForegroundColor Yellow
                $builtAfter = (Get-Date).AddMinutes(-5)
                & pwsh -NoProfile -File (Join-Path $projectRoot 'scripts\export-release.ps1') -BuiltAfter $builtAfter -PortableOnly
            }
        }
    }
}
finally {
    Pop-Location
}
