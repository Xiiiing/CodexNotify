$ErrorActionPreference = 'Stop'

$codexBinRoot = Join-Path $env:LOCALAPPDATA 'OpenAI\Codex\bin'
if (-not (Test-Path -LiteralPath $codexBinRoot)) {
    throw "Codex desktop bin directory was not found: $codexBinRoot"
}

$codexExe = Get-ChildItem -LiteralPath $codexBinRoot -Filter 'codex.exe' -File -Recurse |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1

if (-not $codexExe) {
    throw "The bundled codex.exe was not found under: $codexBinRoot"
}

$reviewDirectory = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))

Write-Host 'Found bundled Codex CLI:' -ForegroundColor Green
Write-Host $codexExe.FullName
Write-Host ''
Write-Host 'When prompted with "Hooks need review":' -ForegroundColor Cyan
Write-Host '  1. Choose "Review hooks" to inspect the command, or'
Write-Host '  2. Choose "Trust all and continue" if this is the only expected hook.'
Write-Host 'Do not choose "Continue without trusting" if you want notifications.'
Write-Host ''

& $codexExe.FullName --no-alt-screen -C $reviewDirectory

