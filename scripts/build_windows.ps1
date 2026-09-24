<#
.SYNOPSIS
    steM. — Windows Automated Build & Packaging Script
    Packages steM. into a standalone Windows x64 distribution using PyInstaller.
.DESCRIPTION
    Created by M. Özçelik for steM.
    Requires Python 3.12+ (x64) and Windows 10/11.
#>

[CmdletBinding()]
param (
    [switch]$SkipVenv,
    [switch]$Clean
)

$ErrorActionPreference = "Stop"

$ProjectDir = Split-Path -Parent $PSScriptRoot
Set-Location $ProjectDir

Write-Host "========================================================" -ForegroundColor Magenta
Write-Host "  steM. — AI Audio Separation Studio Windows Builder" -ForegroundColor Cyan
Write-Host "  Separate the sound. Keep the soul." -ForegroundColor White
Write-Host "  Created by M. Özçelik" -ForegroundColor Gray
Write-Host "========================================================" -ForegroundColor Magenta

# 1. Environment & Python check
$PythonExe = "python.exe"
if (-not $SkipVenv) {
    if (-not (Test-Path ".venv\Scripts\python.exe")) {
        Write-Host "[1/4] Creating Windows Python virtual environment..." -ForegroundColor Yellow
        python -m venv .venv
    }
    $PythonExe = ".venv\Scripts\python.exe"
}

Write-Host "[2/4] Verifying dependencies..." -ForegroundColor Yellow
& $PythonExe -m pip install --upgrade pip
& $PythonExe -m pip install torch torchaudio --index-url https://download.pytorch.org/whl/cu124
& $PythonExe -m pip install demucs soundfile static-ffmpeg numpy scipy pyinstaller

# 2. Clean build directories if requested
if ($Clean) {
    Write-Host "Cleaning build and dist directories..." -ForegroundColor Yellow
    Remove-Item -Recurse -Force -ErrorAction SilentlyContinue build, dist
}

# 3. Package via PyInstaller
Write-Host "[3/4] Compiling steM. executable with PyInstaller..." -ForegroundColor Yellow

$IconPath = Join-Path $ProjectDir "data\icons\com.mozcelik.stem.ico"
if (-not (Test-Path $IconPath)) {
    $IconPath = Join-Path $ProjectDir "data\icons\hicolor\256x256\apps\com.mozcelik.stem.png"
}
$CssPath = Join-Path $ProjectDir "stem\ui\style.css"
$DataPath = Join-Path $ProjectDir "data"

& $PythonExe -m PyInstaller `
    --name="steM" `
    --windowed `
    --icon="$IconPath" `
    --add-data="${CssPath};stem/ui" `
    --add-data="${DataPath};data" `
    --collect-all="demucs" `
    --collect-all="torchaudio" `
    --collect-all="soundfile" `
    --collect-submodules="stem" `
    --noconfirm `
    stem\app.py

# 4. Create release archive
Write-Host "[4/4] Creating distribution archive..." -ForegroundColor Green
$DistFolder = Join-Path $ProjectDir "dist\steM"
$ZipTarget = Join-Path $ProjectDir "dist\steM-Windows-x64.zip"

if (Test-Path $DistFolder) {
    Compress-Archive -Path "$DistFolder\*" -DestinationPath $ZipTarget -Force
    Write-Host "Build complete! Standalone package ready at:" -ForegroundColor Green
    Write-Host "  $ZipTarget" -ForegroundColor Cyan
} else {
    Write-Error "PyInstaller build directory not found."
}

Write-Host "========================================================" -ForegroundColor Magenta
