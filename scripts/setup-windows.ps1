<#
.SYNOPSIS
    Instala o que falta para compilar o b-snip no Windows (Rust + MinGW GNU).
.DESCRIPTION
    Idempotente. Detecta cargo/rustc/gcc/windres e só instala o que não existir.
    Requer Windows 10/11 e, para o MinGW via winget, o App Installer da Microsoft.
#>
. "$PSScriptRoot\_common.ps1"
$Root = Get-BSnipRoot
Set-Location $Root

Write-Host "=== b-snip setup (Windows) ===" -ForegroundColor Cyan

function Ensure-Rust {
    if (Get-Command rustup.exe -ErrorAction SilentlyContinue) {
        Write-Host "rustup ok"
        rustup toolchain install stable --profile minimal
        rustup default stable
        rustup target add x86_64-pc-windows-gnu
        return
    }
    Write-Host "Rust não encontrado. Baixando rustup-init..."
    $tmp = Join-Path $env:TEMP "rustup-init-bsnip.exe"
    Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile $tmp
    & $tmp -y --default-toolchain stable --default-host x86_64-pc-windows-gnu
    $cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
    Add-PathFront $cargoBin
    rustup target add x86_64-pc-windows-gnu
}

function Ensure-Mingw {
    if (Find-MingwBin) {
        Write-Host "MinGW ok:" (Find-MingwBin)
        return
    }
    if (-not (Get-Command winget.exe -ErrorAction SilentlyContinue)) {
        throw "MinGW ausente e winget não está instalado. Instale WinLibs (gcc+windres) e coloque no PATH."
    }
    Write-Host "Instalando WinLibs (MinGW POSIX UCRT) via winget..."
    winget install --id BrechtSanders.WinLibs.POSIX.UCRT -e --accept-package-agreements --accept-source-agreements
    $mingw = Find-MingwBin
    if (-not $mingw) {
        throw "WinLibs instalou, mas gcc/windres não apareceram. Feche o terminal, abra outro e rode este script de novo."
    }
    Write-Host "MinGW ok:" $mingw
}

Ensure-Rust
Ensure-Mingw
$tc = Use-BSnipToolchain

Write-Host ""
Write-Host "Toolchain pronto."
Write-Host "  rustc :" $tc.Rustc
Write-Host "  cargo :" $tc.Cargo
Write-Host "  mingw :" $tc.Mingw
Write-Host "  alvo  :" $tc.Target
Write-Host ""
Write-Host "Próximo passo:  .\scripts\build-release.ps1"
Write-Host "Ou para testar: .\scripts\test.ps1"
