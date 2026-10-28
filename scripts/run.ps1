<#
.SYNOPSIS
    Compila em release e inicia dist\b-snip.exe (encerra instância anterior).
#>
. "$PSScriptRoot\_common.ps1"
$Root = Get-BSnipRoot
& "$PSScriptRoot\build-release.ps1"
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$dest = Join-Path $Root "dist\b-snip.exe"
if (-not (Test-Path $dest)) { throw "dist\b-snip.exe não encontrado" }
Start-Process -FilePath $dest
Write-Host "b-snip iniciado:" $dest
