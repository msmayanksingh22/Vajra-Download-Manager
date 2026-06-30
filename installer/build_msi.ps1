# Vajra Build & MSI Script
# Builds the Tauri desktop app, browser extension, and produces Vajra.msi.
# Run from the project root: .\installer\build_msi.ps1

param(
    [string]$Config = "Release"
)

$ErrorActionPreference = "Stop"
$ProjectRoot = (Get-Item "$PSScriptRoot\..").FullName
$TauriDir = "$ProjectRoot\vajra-ui-tauri"
$TauriConf = "$TauriDir\src-tauri\tauri.conf.json"
$ExtensionDir = "$ProjectRoot\vajra-extension"
$TauriTargetDir = if ($Config -eq "Debug") { "debug" } else { "release" }
# Harvest the Tauri release directory that contains the actual application files
# (Vajra.exe, sidecar, WebView2 loader, etc.), not the bundle/ sub-directory
# that only holds the generated installer artifacts.
# Because this is a Cargo workspace, the target directory is at the project root.
$BundleDir = "$ProjectRoot\target\$TauriTargetDir"

# ── Helper: Generate WXS fragment ─────────────────────────────────────────────
function GenerateFilesWxs($sourceDir, $outputPath) {
    $sb = [System.Text.StringBuilder]::new()
    $null = $sb.AppendLine('<?xml version="1.0" encoding="UTF-8"?>')
    $null = $sb.AppendLine('<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs">')
    $null = $sb.AppendLine('  <Fragment>')
    $null = $sb.AppendLine('    <ComponentGroup Id="AllAppFiles" Directory="INSTALLDIR">')

    $id = 0
    Get-ChildItem $sourceDir -Recurse -File | ForEach-Object {
        $relPath = $_.FullName.Substring($sourceDir.Length + 1)
        $relPathBytes = [System.Text.Encoding]::UTF8.GetBytes($relPath)
        $hashObj = [System.Security.Cryptography.MD5]::Create()
        $hashBytes = $hashObj.ComputeHash($relPathBytes)
        $hash = [BitConverter]::ToString($hashBytes) -replace '-'
        $safeName = "F_$hash"
        $id++

        # Determine sub-directory (if any)
        $relDir = [System.IO.Path]::GetDirectoryName($relPath)
        if ($relDir) {
            # In WiX 4, Subdirectory handles nested paths (e.g. en-US or Assets\Icons)
            $null = $sb.AppendLine("    <!-- $relPath -->")
            $null = $sb.AppendLine("    <Component Id=`"Comp_$safeName`" Directory=`"INSTALLDIR`" Subdirectory=`"$relDir`">")
            $null = $sb.AppendLine("      <File Id=`"File_$safeName`" Source=`"$($_.FullName)`" KeyPath=`"yes`" />")
            $null = $sb.AppendLine("    </Component>")
        } else {
            $null = $sb.AppendLine("    <Component Id=`"Comp_$safeName`">")
            $null = $sb.AppendLine("      <File Id=`"File_$safeName`" Source=`"$($_.FullName)`" KeyPath=`"yes`" />")
            $null = $sb.AppendLine("    </Component>")
        }
    }

    $null = $sb.AppendLine('    </ComponentGroup>')
    $null = $sb.AppendLine('  </Fragment>')
    $null = $sb.AppendLine('</Wix>')

    Set-Content $outputPath $sb.ToString() -Encoding UTF8
    Write-Host "  Generated file list: $((Get-ChildItem $sourceDir -Recurse -File).Count) files"
}

# ── Read Tauri version (MSI source of truth) ──────────────────────────────────
if (-not (Test-Path $TauriConf)) {
    throw "tauri.conf.json not found at $TauriConf"
}
$tauriJson = Get-Content $TauriConf -Raw | ConvertFrom-Json
$Version = $tauriJson.version
if (-not $Version) {
    throw "Could not read version from $TauriConf"
}

Write-Host ""
Write-Host "=== Vajra MSI Build Script ===" -ForegroundColor Cyan
Write-Host "Project root : $ProjectRoot"
Write-Host "Tauri dir    : $TauriDir"
Write-Host "Extension dir: $ExtensionDir"
Write-Host "Bundle dir   : $BundleDir"
Write-Host "Version      : $Version"
Write-Host "Config       : $Config"
Write-Host ""

# ── Step 1: Build the Tauri app ────────────────────────────────────────────────
Write-Host "[1/4] Building Tauri app ($Config)..." -ForegroundColor Yellow
Push-Location $TauriDir
try {
    if ($Config -eq "Debug") {
        npm run tauri build -- --debug
    } else {
        npm run tauri build
    }
    if ($LASTEXITCODE -ne 0) { throw "Tauri build failed" }
} finally {
    Pop-Location
}
Write-Host "  Tauri app built" -ForegroundColor Green

# ── Step 2: Build the browser extension ──────────────────────────────────────
Write-Host ""
Write-Host "[2/4] Building browser extension..." -ForegroundColor Yellow
Push-Location $ExtensionDir
try {
    npm run build
    if ($LASTEXITCODE -ne 0) { throw "Extension build failed" }
} finally {
    Pop-Location
}
Write-Host "  Extension built" -ForegroundColor Green

# ── Step 3: Stage installer payload ──────────────────────────────────────────
Write-Host ""
Write-Host "[3/4] Packaging extension and staging installer payload..." -ForegroundColor Yellow
if (-not (Test-Path $BundleDir)) {
    throw "Tauri release output not found at $BundleDir"
}

$StageDir = "$PSScriptRoot\..\installer\staging"
if (Test-Path $StageDir) { Remove-Item -Recurse -Force $StageDir }
New-Item -ItemType Directory -Force -Path $StageDir | Out-Null
$StageDir = (Get-Item $StageDir).FullName

# Copy the Tauri app binary, sidecar, and any required DLLs.
# We intentionally exclude Rust build internals (build/, deps/, .fingerprint/, etc.)
# and unrelated test/CLI binaries.
$RequiredFiles = @('vajra-ui-tauri.exe', 'vajrad.exe')
Get-ChildItem -Path $BundleDir -File | Where-Object {
    $_.Name -in $RequiredFiles -or $_.Extension -eq '.dll'
} | ForEach-Object {
    Copy-Item $_.FullName $StageDir -Force
}

# Copy bundled resources (icon, sidecar manifest, etc.) if present.
$ResourceSrc = "$BundleDir\resources"
if (Test-Path $ResourceSrc) {
    Copy-Item $ResourceSrc "$StageDir\resources" -Recurse -Force
}

# Package the browser extension.
$ExtDest = "$StageDir\resources\extension"
if (Test-Path $ExtDest) { Remove-Item -Recurse -Force $ExtDest }
Copy-Item -Path "$ExtensionDir\dist" -Destination $ExtDest -Recurse -Force
Write-Host "  Payload staged at $StageDir" -ForegroundColor Green

# ── Step 4: Generate WiX file list & build MSI ───────────────────────────────
Write-Host ""
Write-Host "[4/4] Generating file list and building MSI..." -ForegroundColor Yellow

$wxsPath = "$PSScriptRoot\VajraFiles.wxs"
GenerateFilesWxs $StageDir $wxsPath

$wix = "$env:USERPROFILE\.dotnet\tools\wix.exe"
if (-not (Test-Path $wix)) { throw "WiX not found. Run: dotnet tool install -g wix" }

Push-Location $PSScriptRoot
try {
    & $wix build "Vajra.wxs" "VajraFiles.wxs" `
        -o "Vajra.msi" `
        -arch x64 `
        -ext WixToolset.UI.wixext `
        -ext WixToolset.Util.wixext `
        -d "Version=$Version"
}
finally {
    Pop-Location
}
if ($LASTEXITCODE -ne 0) { throw "WiX build failed" }

Remove-Item $wxsPath -ErrorAction SilentlyContinue
if (Test-Path $StageDir) { Remove-Item -Recurse -Force $StageDir -ErrorAction SilentlyContinue }

$msiPath = "$PSScriptRoot\Vajra.msi"
$msiSize = [math]::Round((Get-Item $msiPath).Length / 1MB, 1)
Write-Host ""
Write-Host "Vajra.msi built successfully! ($msiSize MB)" -ForegroundColor Green
Write-Host "   Path: $msiPath" -ForegroundColor Cyan
Write-Host "   Version: $Version" -ForegroundColor Cyan
