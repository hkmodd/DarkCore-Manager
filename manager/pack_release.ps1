$ErrorActionPreference = "Stop"

Write-Host "========= DARKCORE RELEASE PACKER =========" -ForegroundColor Magenta
Write-Host "🔨 1. Building Release Binary..." -ForegroundColor Cyan
cargo build --release

$releaseDir = Join-Path $PSScriptRoot "dist"
if (Test-Path $releaseDir) { 
    Write-Host "🧹 Cleaning old dist..." -ForegroundColor Yellow
    Remove-Item $releaseDir -Recurse -Force 
}
New-Item -ItemType Directory -Path $releaseDir | Out-Null

Write-Host "📦 2. Copying Safe Files..." -ForegroundColor Cyan

# Use absolute paths to avoid confusion
Copy-Item (Join-Path $PSScriptRoot "target\release\darkcore-greenluma.exe") -Destination $releaseDir
Copy-Item (Join-Path $PSScriptRoot "README.md") -Destination $releaseDir
Copy-Item (Join-Path $PSScriptRoot "LICENSE") -Destination $releaseDir

$zipName = Join-Path $PSScriptRoot "DarkCore_v10.4_Release.zip"
if (Test-Path $zipName) { Remove-Item $zipName -Force }

Write-Host "🤐 3. Zipping Artifact..." -ForegroundColor Cyan
Compress-Archive -Path "$releaseDir\*" -DestinationPath $zipName

Write-Host "✅ SUCCESS!" -ForegroundColor Green
Write-Host "   - Source Code: Ready."
Write-Host "   - Binary Release: $zipName"
