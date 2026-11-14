# build.ps1 - Build & Install Omore Punch (VST3 & CLAP) on Windows
$ErrorActionPreference = "Stop"

Write-Host "=================================================" -ForegroundColor Cyan
Write-Host "   Omore Punch - Build VST3 & CLAP (Windows)     " -ForegroundColor Cyan
Write-Host "=================================================" -ForegroundColor Cyan

# 1. Execution des tests unitaires DSP
Write-Host "`n[1/3] Execution des tests unitaires DSP (Release)..." -ForegroundColor Cyan
cargo test --release
if ($LASTEXITCODE -ne 0) {
    Write-Error "Echec des tests unitaires DSP."
    exit $LASTEXITCODE
}

# 2. Bundling VST3 & CLAP via xtask
Write-Host "`n[2/3] Bundling VST3 & CLAP via xtask..." -ForegroundColor Cyan
cargo run --package xtask --release -- bundle omore-punch --release
if ($LASTEXITCODE -ne 0) {
    Write-Error "Echec du bundling xtask."
    exit $LASTEXITCODE
}

# 3. Determination des dossiers de destination VST3 et CLAP
function Test-WritableDirectory([string]$dirPath) {
    try {
        if (-not (Test-Path $dirPath)) {
            New-Item -ItemType Directory -Path $dirPath -Force -ErrorAction Stop | Out-Null
        }
        $testFile = Join-Path $dirPath ([System.Guid]::NewGuid().ToString() + ".tmp")
        [System.IO.File]::WriteAllText($testFile, "test")
        Remove-Item -LiteralPath $testFile -Force -ErrorAction SilentlyContinue
        return $true
    } catch {
        return $false
    }
}

$systemVst3Root = "C:\Program Files\Common Files\VST3"
$userVst3Root   = "$env:LOCALAPPDATA\Programs\Common\VST3"
$systemClapRoot = "C:\Program Files\Common Files\CLAP"
$userClapRoot   = "$env:LOCALAPPDATA\Programs\Common\CLAP"

if (Test-WritableDirectory $systemVst3Root) {
    $targetVst3Root = $systemVst3Root
    Write-Host "`n[3/3] Destination VST3 : Systeme ($targetVst3Root)" -ForegroundColor Green
} else {
    $targetVst3Root = $userVst3Root
    New-Item -ItemType Directory -Path $targetVst3Root -Force | Out-Null
    Write-Host "`n[3/3] Destination VST3 : Utilisateur ($targetVst3Root)" -ForegroundColor Yellow
}

if (Test-WritableDirectory $systemClapRoot) {
    $targetClapRoot = $systemClapRoot
    Write-Host "      Destination CLAP : Systeme ($targetClapRoot)" -ForegroundColor Green
} else {
    $targetClapRoot = $userClapRoot
    New-Item -ItemType Directory -Path $targetClapRoot -Force | Out-Null
    Write-Host "      Destination CLAP : Utilisateur ($targetClapRoot)" -ForegroundColor Yellow
}

$destVst3 = Join-Path $targetVst3Root "omore-punch.vst3"
$destClap = Join-Path $targetClapRoot "omore-punch.clap"

try {
    if (Test-Path $destVst3) {
        Remove-Item -Recurse -Force $destVst3 -ErrorAction Stop
    }
    Copy-Item -Recurse -Force "target\bundled\omore-punch.vst3" $destVst3 -ErrorAction Stop

    if (Test-Path $destClap) {
        Remove-Item -Recurse -Force $destClap -ErrorAction Stop
    }
    Copy-Item -Recurse -Force "target\bundled\omore-punch.clap" $destClap -ErrorAction Stop
} catch {
    Write-Error "Impossible de copier les bundles. Verifiez que Reaper n'est pas en cours d'execution avec le plugin charge !"
    exit 1
}

Write-Host "`nBundles installes avec succes :" -ForegroundColor Green
Write-Host "  - VST3 : $destVst3" -ForegroundColor Green
Write-Host "  - CLAP : $destClap" -ForegroundColor Green

# Ce script ne fait aucune operation Git : les commits se font separement (GEMINI.md).
Write-Host "`nTermine !" -ForegroundColor Green

