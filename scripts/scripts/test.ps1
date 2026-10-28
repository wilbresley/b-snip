<#
.SYNOPSIS
    Roda os testes unitários do b-snip.
#>
. "$PSScriptRoot\_common.ps1"
$Root = Get-BSnipRoot
Set-Location $Root
$tc = Use-BSnipToolchain
cargo test --target $tc.Target @args
exit $LASTEXITCODE
