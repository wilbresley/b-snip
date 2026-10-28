# Shared helpers for b-snip build scripts. Dot-source from the other scripts.

$ErrorActionPreference = "Stop"

function Get-BSnipRoot {
    Split-Path -Parent $PSScriptRoot
}

function Add-PathFront([string]$dir) {
    if ($dir -and (Test-Path $dir) -and ($env:Path -notlike "*$dir*")) {
        $env:Path = "$dir;" + $env:Path
    }
}

function Find-MingwBin {
    $names = @("gcc.exe", "windres.exe")
    function Test-Dir($d) {
        if (-not $d) { return $false }
        foreach ($n in $names) {
            if (-not (Test-Path (Join-Path $d $n))) { return $false }
        }
        return $true
    }

    $fromPath = (Get-Command gcc.exe -ErrorAction SilentlyContinue | Select-Object -First 1).Source
    if ($fromPath) {
        $dir = Split-Path $fromPath
        if (Test-Dir $dir) { return $dir }
    }

    $candidates = @(
        "$env:USERPROFILE\mingw64\bin",
        "C:\mingw64\bin",
        "C:\msys64\mingw64\bin",
        "C:\ProgramData\mingw64\mingw64\bin"
    )
    $wingetRoot = Join-Path $env:LOCALAPPDATA "Microsoft\WinGet\Packages"
    if (Test-Path $wingetRoot) {
        Get-ChildItem -Path $wingetRoot -Directory -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -like "BrechtSanders.WinLibs*" -or $_.Name -like "*mingw*" } |
            ForEach-Object {
                $candidates += (Join-Path $_.FullName "mingw64\bin")
            }
    }
    foreach ($d in $candidates) {
        if (Test-Dir $d) { return $d }
    }
    return $null
}

function Use-BSnipToolchain {
    $mingw = Find-MingwBin
    if (-not $mingw) {
        throw @"
MinGW (gcc + windres) não encontrado.
Instale o toolchain GNU e rode de novo:

  powershell -ExecutionPolicy Bypass -File .\scripts\setup-windows.ps1

Ou instale WinLibs manualmente e deixe gcc/windres no PATH.
"@
    }
    Add-PathFront $mingw

    $cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
    Add-PathFront $cargoBin

    if (-not (Get-Command cargo.exe -ErrorAction SilentlyContinue)) {
        throw "cargo não encontrado. Instale Rust via https://rustup.rs e rode scripts\setup-windows.ps1"
    }
    if (-not (Get-Command rustc.exe -ErrorAction SilentlyContinue)) {
        throw "rustc não encontrado. Instale Rust via https://rustup.rs"
    }

    [pscustomobject]@{
        Mingw   = $mingw
        Cargo   = (Get-Command cargo.exe).Source
        Rustc   = (Get-Command rustc.exe).Source
        Target  = "x86_64-pc-windows-gnu"
    }
}

function Stop-BSnipProcesses {
    foreach ($name in @("b-snip", "b-abcABC", "WBCApp")) {
        Get-Process -Name $name -ErrorAction SilentlyContinue | Stop-Process -Force
    }
}

function Get-BSnipExePath {
    param([string]$Root, [string]$Profile = "release")
    $targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $Root "target" }
    $gnu = Join-Path $targetDir "x86_64-pc-windows-gnu\$Profile\b-snip.exe"
    $native = Join-Path $targetDir "$Profile\b-snip.exe"
    if (Test-Path $gnu) { return $gnu }
    if (Test-Path $native) { return $native }
    return $null
}
