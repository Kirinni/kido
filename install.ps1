#Requires -Version 5.1
<#
.SYNOPSIS
    安装 kido —— 终端待办。单文件 exe，只往 %LOCALAPPDATA%\kido 放一个目录。

.DESCRIPTION
    下载 GitHub Release 里的预编译 exe，安装到 $Dest，并把该目录加进用户 PATH。
    卸载用 `kido --uninstall`，或删掉这个目录再清一下 PATH。

.EXAMPLE
    irm https://raw.githubusercontent.com/Kirinni/kido/main/install.ps1 | iex

.EXAMPLE
    .\install.ps1 -Version v0.1.0

.EXAMPLE
    .\install.ps1 -Uninstall
#>
[CmdletBinding()]
param(
    [string]$Repo = 'Kirinni/kido',
    [string]$Version = 'latest',
    [string]$Dest = (Join-Path $env:LOCALAPPDATA 'kido'),
    [switch]$NoPath,
    [switch]$Uninstall,
    [switch]$KeepData
)

$ErrorActionPreference = 'Stop'
try {
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
} catch {
    # PowerShell 7 已经默认用 TLS 1.2+，忽略即可
}

$Asset = 'kido-x86_64-pc-windows-msvc.exe'
$Exe = Join-Path $Dest 'kido.exe'
$DataFile = Join-Path $Dest 'todo.txt'

function Get-UserPathText {
    $key = Get-Item 'HKCU:\Environment'
    $value = $key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
    return [string]$value
}

function Get-UserPathKind {
    $key = Get-Item 'HKCU:\Environment'
    try {
        return $key.GetValueKind('Path').ToString()
    } catch {
        return 'ExpandString'
    }
}

function Test-InUserPath([string]$Dir) {
    $target = $Dir.TrimEnd('\')
    foreach ($entry in (Get-UserPathText) -split ';') {
        if ($entry -ne '' -and $entry.TrimEnd('\') -ieq $target) { return $true }
    }
    return $false
}

function Add-UserPath([string]$Dir) {
    if (Test-InUserPath $Dir) { return $false }
    $kind = Get-UserPathKind
    $entries = @((Get-UserPathText) -split ';' | Where-Object { $_ -ne '' })
    $new = (@($entries) + $Dir) -join ';'
    New-ItemProperty -Path 'HKCU:\Environment' -Name 'Path' -Value $new -PropertyType $kind -Force | Out-Null
    Send-SettingChange
    return $true
}

function Remove-UserPath([string]$Dir) {
    $target = $Dir.TrimEnd('\')
    $entries = @((Get-UserPathText) -split ';' | Where-Object { $_ -ne '' })
    $kept = @($entries | Where-Object { $_.TrimEnd('\') -ine $target })
    if ($kept.Count -eq $entries.Count) { return $false }
    $kind = Get-UserPathKind
    New-ItemProperty -Path 'HKCU:\Environment' -Name 'Path' -Value ($kept -join ';') -PropertyType $kind -Force | Out-Null
    Send-SettingChange
    return $true
}

function Send-SettingChange {
    if (-not ('Kido.Native' -as [type])) {
        Add-Type -Namespace Kido -Name Native -MemberDefinition @'
[DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Auto)]
public static extern IntPtr SendMessageTimeout(IntPtr hWnd, uint Msg, UIntPtr wParam, string lParam, uint fuFlags, uint uTimeout, out UIntPtr lpdwResult);
'@
    }
    $result = [UIntPtr]::Zero
    [Kido.Native]::SendMessageTimeout([IntPtr]0xffff, 0x1A, [UIntPtr]::Zero, 'Environment', 2, 3000, [ref]$result) | Out-Null
}

function Test-KidoRunning {
    return @(Get-Process kido -ErrorAction SilentlyContinue).Count -gt 0
}

# ------------------------------------------------------------------ 卸载

if ($Uninstall) {
    if (Test-KidoRunning) {
        Write-Host 'kido 正在运行，请先退出（在程序里按 q）再卸载。' -ForegroundColor Yellow
        exit 1
    }
    $removedPath = Remove-UserPath $Dest
    if (Test-Path $Dest) {
        if ($KeepData -and (Test-Path $DataFile)) {
            Remove-Item (Join-Path $Dest 'kido.exe') -Force -ErrorAction SilentlyContinue
            Write-Host "已删除程序，保留数据：$DataFile" -ForegroundColor Green
        } else {
            Remove-Item $Dest -Recurse -Force
            Write-Host "已删除目录：$Dest" -ForegroundColor Green
        }
    }
    Write-Host ("PATH：" + $(if ($removedPath) { '已移除' } else { '无相关条目' }))
    exit 0
}

# ------------------------------------------------------------------ 安装

if ($Version -eq 'latest') {
    $baseUrl = "https://github.com/$Repo/releases/latest/download"
} else {
    $baseUrl = "https://github.com/$Repo/releases/download/$Version"
}

Write-Host "从 $baseUrl 下载 $Asset ..." -ForegroundColor Cyan
$tempExe = Join-Path $env:TEMP "kido-$([guid]::NewGuid().ToString('N')).exe"
try {
    Invoke-WebRequest -Uri "$baseUrl/$Asset" -OutFile $tempExe -UseBasicParsing
} catch {
    Write-Host "下载失败：$_" -ForegroundColor Red
    Write-Host "如果仓库还没发布 Release，请先执行：git tag v0.1.0; git push origin v0.1.0" -ForegroundColor Yellow
    exit 1
}

$expectedHash = $null
try {
    $hashFile = (Invoke-WebRequest -Uri "$baseUrl/$Asset.sha256" -UseBasicParsing).Content.Trim()
    $expectedHash = ($hashFile -split '\s+')[0].ToLower()
} catch {
    Write-Host '没有找到 sha256 校验文件，跳过校验。' -ForegroundColor Yellow
}
if ($expectedHash) {
    $actual = (Get-FileHash $tempExe -Algorithm SHA256).Hash.ToLower()
    if ($actual -ne $expectedHash) {
        Remove-Item $tempExe -Force
        Write-Host "校验失败：期望 $expectedHash，实际 $actual" -ForegroundColor Red
        exit 1
    }
    Write-Host 'sha256 校验通过。' -ForegroundColor Green
}

if (Test-KidoRunning) {
    Remove-Item $tempExe -Force
    Write-Host 'kido 正在运行，请先退出（在程序里按 q）再安装。' -ForegroundColor Yellow
    exit 1
}

New-Item -ItemType Directory -Force -Path $Dest | Out-Null
if (Test-Path $Exe) { Remove-Item $Exe -Force }
Move-Item -Path $tempExe -Destination $Exe -Force

$version = (& $Exe --version) -join ''
Write-Host "已安装 $version -> $Exe" -ForegroundColor Green

if ($NoPath) {
    Write-Host "跳过 PATH（该目录是 $Dest）"
} else {
    $added = Add-UserPath $Dest
    if ($added) {
        Write-Host "已把 $Dest 加入用户 PATH。" -ForegroundColor Green
    } else {
        Write-Host "$Dest 已经在用户 PATH 里。"
    }
    Write-Host ''
    Write-Host '注意：如果 Windows Terminal / VS Code 已经在运行，它们不会自动继承新的 PATH。' -ForegroundColor Yellow
    Write-Host '      请完全退出这些程序（或注销一次），之后新开的终端里直接输入 kido 即可。' -ForegroundColor Yellow
}

Write-Host ''
Write-Host "数据文件：$(Join-Path $Dest 'todo.txt')"
Write-Host '卸载：kido --uninstall   （保留数据加 -k）'
