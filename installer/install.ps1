# Vajra Download Manager — Install / Uninstall Script
# Run as Administrator for a system-wide install, or run unelevated for a per-user install.

param(
    [string]$Config = "Release",
    [switch]$Uninstall
)

$VajraDir = "C:\Program Files\Vajra"
$RegKey_Run = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"

if ($Uninstall) {
    Write-Host "Uninstalling Vajra..." -ForegroundColor Yellow

    Remove-ItemProperty $RegKey_Run "VajraDownloadManager" -ErrorAction SilentlyContinue
    Remove-Item $VajraDir -Recurse -Force -ErrorAction SilentlyContinue

    Write-Host "Vajra uninstalled." -ForegroundColor Green
    exit 0
}

# ─── Install ──────────────────────────────────────────────────────────────────

Write-Host "Installing Vajra Download Manager..." -ForegroundColor Cyan

# Locate the Tauri release output
$ProjectRoot = (Get-Item "$PSScriptRoot\..").FullName
$TauriTargetDir = if ($Config -eq "Debug") { "debug" } else { "release" }
# Cargo workspace output is at the project root target directory.
$BuildDir = "$ProjectRoot\target\$TauriTargetDir"

if (-not (Test-Path $BuildDir)) {
    Write-Error "Tauri build output not found at $BuildDir — please run build_msi.ps1 first"
    exit 1
}

# Create install directory
New-Item -ItemType Directory -Force -Path $VajraDir | Out-Null

# Copy the Tauri application binary, daemon sidecar, and any required DLLs.
# We skip unrelated test/CLI binaries from the Cargo workspace output.
$RequiredFiles = @('vajra-ui-tauri.exe', 'vajrad.exe')
Get-ChildItem -Path $BuildDir -File | Where-Object {
    $_.Name -in $RequiredFiles -or $_.Extension -eq '.dll'
} | ForEach-Object {
    Copy-Item $_.FullName $VajraDir -Force
}

# Copy bundled resources (e.g. browser extension) if present
$ResourceSrc = "$BuildDir\resources"
if (Test-Path $ResourceSrc) {
    Copy-Item $ResourceSrc "$VajraDir\resources" -Recurse -Force
    Write-Host "Copied bundled resources" -ForegroundColor Green
}

# Ensure the main executable exists
$exePath = "$VajraDir\vajra-ui-tauri.exe"
if (-not (Test-Path $exePath)) {
    Write-Error "Main executable not found at $exePath"
    exit 1
}

# Add to startup (optional)
Set-ItemProperty -Path $RegKey_Run -Name "VajraDownloadManager" -Value "`"$exePath`" --minimized" -ErrorAction SilentlyContinue
Write-Host "Added to startup" -ForegroundColor Green

# Create desktop shortcut
$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut("$env:USERPROFILE\Desktop\Vajra Download Manager.lnk")
$shortcut.TargetPath = $exePath
$shortcut.WorkingDirectory = $VajraDir
$shortcut.Description = "Vajra Download Manager"
$shortcut.Save()
Write-Host "Created desktop shortcut" -ForegroundColor Green

Write-Host ""
Write-Host "Vajra Download Manager installed successfully!" -ForegroundColor Green
Write-Host "Launch from: $exePath" -ForegroundColor Cyan
