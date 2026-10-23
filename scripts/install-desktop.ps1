# Build and install StepX Companion for the current Windows user.
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$CargoBin = Join-Path $HOME ".cargo\bin"
$env:PATH = "$CargoBin;$env:PATH"
Set-Location (Join-Path $Root "desktop")
cargo build --locked

$InstallDir = if ($env:STEPX_INSTALL_DIR) { $env:STEPX_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "Programs\StepXCompanion" }
New-Item -ItemType Directory -Force -Path $InstallDir, (Join-Path $InstallDir "tools") | Out-Null
Copy-Item "target\debug\stepx-companion.exe" (Join-Path $InstallDir "stepx-companion.exe") -Force

$LocalTools = Join-Path $Root ".tools\bin"
if (Test-Path $LocalTools) {
    Copy-Item $LocalTools (Join-Path $InstallDir "tools\bin") -Recurse -Force
}

$StartMenu = [Environment]::GetFolderPath("Programs")
$ShortcutPath = Join-Path $StartMenu "阶跃伴侣 (StepX Companion).lnk"
$WshShell = New-Object -ComObject WScript.Shell
$Shortcut = $WshShell.CreateShortcut($ShortcutPath)
$Shortcut.TargetPath = Join-Path $InstallDir "stepx-companion.exe"
$Shortcut.WorkingDirectory = $InstallDir
$Shortcut.Description = "连接 StepX，协同手机与电脑"
$Shortcut.Save()

Write-Host "已安装至：$InstallDir\stepx-companion.exe（已创建开始菜单快捷方式）"
