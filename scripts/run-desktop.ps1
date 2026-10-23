# Run StepX Companion on Windows (PowerShell)
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$ToolsBin = Join-Path $Root ".tools\bin"
$CargoBin = Join-Path $HOME ".cargo\bin"
$env:PATH = "$CargoBin;$ToolsBin;$env:PATH"
Set-Location (Join-Path $Root "desktop")
cargo run --bin stepx-companion -- $args
