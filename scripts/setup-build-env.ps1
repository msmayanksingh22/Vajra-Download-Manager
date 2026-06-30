# Run this script as Administrator to fix build issues
# Right-click → Run as Administrator

Write-Host "Vajra Build Environment Setup" -ForegroundColor Cyan
Write-Host "=============================" -ForegroundColor Cyan
Write-Host ""

# 1. Add Windows Defender exclusions for the build directory
Write-Host "[1/2] Adding Windows Defender exclusions..." -ForegroundColor Yellow
try {
    Add-MpPreference -ExclusionPath "D:\Project\Project-Vajra\target"
    Add-MpPreference -ExclusionPath "D:\Project\Project-Vajra\vajra-ui\obj"
    Add-MpPreference -ExclusionPath "D:\Project\Project-Vajra\vajra-ui\bin"
    Add-MpPreference -ExclusionProcess "cargo.exe"
    Add-MpPreference -ExclusionProcess "rustc.exe"
    Add-MpPreference -ExclusionProcess "link.exe"
    Write-Host "  ✅ Defender exclusions added" -ForegroundColor Green
} catch {
    Write-Host "  ❌ Failed (run as Administrator): $_" -ForegroundColor Red
}

# 2. Check if Visual Studio is installed
Write-Host ""
Write-Host "[2/2] Checking Visual Studio installation..." -ForegroundColor Yellow
$vsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (Test-Path $vsWhere) {
    $vs = & $vsWhere -latest -format json | ConvertFrom-Json
    Write-Host "  ✅ Visual Studio found: $($vs.displayName)" -ForegroundColor Green
} else {
    Write-Host "  ❌ Visual Studio not found!" -ForegroundColor Red
    Write-Host ""
    Write-Host "  REQUIRED: Install Visual Studio 2022 Community (FREE)" -ForegroundColor Yellow
    Write-Host "  Download: https://visualstudio.microsoft.com/vs/community/" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "  Required workloads during installation:" -ForegroundColor Yellow
    Write-Host "    ✓ .NET desktop development" -ForegroundColor White
    Write-Host "    ✓ Windows application development" -ForegroundColor White
    Write-Host ""
    Write-Host "  Opening download page..." -ForegroundColor Yellow
    Start-Process "https://visualstudio.microsoft.com/vs/community/"
}

Write-Host ""
Write-Host "After installing Visual Studio:" -ForegroundColor Cyan
Write-Host "  1. Open D:\Project\Project-Vajra\vajra-ui\Vajra.csproj in Visual Studio" -ForegroundColor White
Write-Host "  2. Press Ctrl+Shift+B to build" -ForegroundColor White
Write-Host "  3. Press F5 to run" -ForegroundColor White
