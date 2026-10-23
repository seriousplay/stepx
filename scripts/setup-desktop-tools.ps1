# Set up local ADB and scrcpy tools on Windows into .tools\bin without requiring administrator rights.
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$Tools = Join-Path $Root ".tools"
$Packages = Join-Path $Tools "packages"
$Bin = Join-Path $Tools "bin"
New-Item -ItemType Directory -Force -Path $Packages, $Bin | Out-Null

$ZipPath = Join-Path $Packages "platform-tools-latest-windows.zip"
Write-Host "Downloading official Android Platform-Tools for Windows..."
Invoke-WebRequest -Uri "https://dl.google.com/android/repository/platform-tools-latest-windows.zip" -OutFile $ZipPath
Expand-Archive -Path $ZipPath -DestinationPath $Tools -Force
Copy-Item (Join-Path $Tools "platform-tools\*") $Bin -Recurse -Force

if (-not (Get-Command scrcpy.exe -ErrorAction SilentlyContinue)) {
    Write-Host "Tip: Install scrcpy via 'winget install Genymobile.scrcpy' or 'scoop install scrcpy' for real-time screen mirroring."
}
& (Join-Path $Bin "adb.exe") version
Write-Host "StepX Companion Windows tools are ready in $Bin"
