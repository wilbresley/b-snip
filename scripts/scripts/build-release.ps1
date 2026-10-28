<#
.SYNOPSIS
    Compila o b-snip em release (um EXE, GNU, LTO) e copia para dist\.
#>
param(
    [switch]$CleanUserData
)

. "$PSScriptRoot\_common.ps1"
$Root = Get-BSnipRoot
Set-Location $Root
$tc = Use-BSnipToolchain

Write-Host "=== b-snip build release ===" -ForegroundColor Cyan
Write-Host "  mingw:" $tc.Mingw

Stop-BSnipProcesses
Start-Sleep -Milliseconds 400

if ($CleanUserData) {
    $userDir = Join-Path $env:LOCALAPPDATA "b-snip"
    if (Test-Path $userDir) {
        Write-Host "Limpando dados de teste em" $userDir
        Remove-Item $userDir -Recurse -Force
    }
}

cargo build --release --target $tc.Target
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$exe = Get-BSnipExePath -Root $Root -Profile "release"
if (-not $exe) { throw "b-snip.exe não apareceu após o cargo build" }

$outDir = Join-Path $Root "dist"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
$dest = Join-Path $outDir "b-snip.exe"
Copy-Item $exe $dest -Force
foreach ($old in @("b-abcABC.exe", "WBCApp.exe")) {
    Remove-Item (Join-Path $outDir $old) -ErrorAction SilentlyContinue
}

$ie4 = Join-Path $env:SystemRoot "System32\ie4uinit.exe"
if (Test-Path $ie4) {
    & $ie4 -show 2>$null
}

Write-Host ""
Write-Host "EXE pronto:" $dest
Get-Item $dest | Format-List FullName, Length, LastWriteTime
