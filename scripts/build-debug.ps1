<#
.SYNOPSIS
    Compila o b-snip em debug (mais rápido, com símbolos).
#>
. "$PSScriptRoot\_common.ps1"
$Root = Get-BSnipRoot
Set-Location $Root
$tc = Use-BSnipToolchain
cargo build --target $tc.Target
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$exe = Get-BSnipExePath -Root $Root -Profile "debug"
Write-Host "Debug:" $exe
