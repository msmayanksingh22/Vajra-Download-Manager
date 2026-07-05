@echo off
setlocal EnableDelayedExpansion

echo.
echo ==========================================
echo   VAJRA - Full Build Script
echo ==========================================
echo.

:: ── Parse Build Type ────────────────────────────────────────────────────────
set "BUILD_TYPE=release"
set "CARGO_FLAGS=--release"
set "TAURI_FLAGS="

if "%~1"=="--debug" (
    set "BUILD_TYPE=debug"
    set "CARGO_FLAGS="
    set "TAURI_FLAGS=-- --debug"
)

:: ── Locate MSVC ──────────────────────────────────────────────────────────────
:: We know VS18 Community is installed on this machine - check it first
set "VCVARS="
if exist "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvars64.bat" (
    set "VCVARS=C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvars64.bat"
    goto :found_vc
)
if exist "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat" (
    set "VCVARS=C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat"
    goto :found_vc
)
if exist "C:\Program Files\Microsoft Visual Studio\2022\Professional\VC\Auxiliary\Build\vcvars64.bat" (
    set "VCVARS=C:\Program Files\Microsoft Visual Studio\2022\Professional\VC\Auxiliary\Build\vcvars64.bat"
    goto :found_vc
)
if exist "C:\Program Files\Microsoft Visual Studio\2022\Enterprise\VC\Auxiliary\Build\vcvars64.bat" (
    set "VCVARS=C:\Program Files\Microsoft Visual Studio\2022\Enterprise\VC\Auxiliary\Build\vcvars64.bat"
    goto :found_vc
)
if exist "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" (
    set "VCVARS=C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
    goto :found_vc
)
echo [ERROR] Visual Studio / MSVC not found. Install VS Community from:
echo         https://visualstudio.microsoft.com/downloads/
pause & exit /b 1

:found_vc
call "%VCVARS%" >nul 2>&1
echo [OK] MSVC loaded.

:: ── Check Rust ───────────────────────────────────────────────────────────────
where cargo >nul 2>&1
if errorlevel 1 (
    echo [ERROR] Rust not found. Install from https://rustup.rs
    pause & exit /b 1
)
echo [OK] Rust found.

:: ── Check Node ───────────────────────────────────────────────────────────────
where npm >nul 2>&1
if errorlevel 1 (
    echo [ERROR] Node.js not found. Install from https://nodejs.org
    pause & exit /b 1
)
echo [OK] Node.js found.
echo.

:: ── Build Rust crates ────────────────────────────────────────────────────────
echo [1/3] Building Rust crates in !BUILD_TYPE! mode...
taskkill /F /IM vajrad.exe >nul 2>&1
cargo build !CARGO_FLAGS! -p vajra-engine -p vajra-protocol -p vajra-daemon -p vajra-cli 2>&1
if errorlevel 1 (
    echo [FAIL] Rust build failed.
    pause & exit /b 1
)
echo [OK] Rust crates built.
echo.

:: ── Copy sidecar daemon ──────────────────────────────────────────────────────
echo Copying sidecar daemon for Tauri packaging...
copy /Y "target\!BUILD_TYPE!\vajrad.exe" "vajra-ui-tauri\src-tauri\vajrad-x86_64-pc-windows-msvc.exe" >nul 2>&1
if errorlevel 1 (
    echo [FAIL] Copying sidecar failed.
    pause & exit /b 1
)
echo [OK] Sidecar copied.

:: ── Install npm deps if needed ───────────────────────────────────────────────
echo [2/3] Checking npm dependencies...
if not exist "vajra-ui-tauri\node_modules" (
    echo Installing npm packages...
    cd vajra-ui-tauri && npm install 2>&1 && cd ..
    if errorlevel 1 (
        echo [FAIL] npm install failed.
        pause & exit /b 1
    )
)
echo [OK] npm dependencies ready.
echo.

:: ── Build Tauri UI ───────────────────────────────────────────────────────────
echo [3/4] Building Tauri UI...
cd vajra-ui-tauri
npm run tauri build !TAURI_FLAGS! 2>&1
if errorlevel 1 (
    cd ..
    echo [FAIL] Tauri build failed.
    pause & exit /b 1
)
cd ..
echo [OK] Tauri UI built.
echo.

:: -- Build Extension ---------------------------------------------------------------
echo [4/4] Building browser extension...
cd vajra-extension
npm run build 2>&1
if errorlevel 1 (
    cd ..
    echo [WARN] Extension build failed - skipping.
) else (
    cd ..
    :: Package the dist folder as a zip for distribution
    powershell -NoProfile -Command "Compress-Archive -Path 'vajra-extension\dist\*' -DestinationPath 'vajra-extension\vajra-extension.zip' -Force"
    echo [OK] Extension built: vajra-extension\vajra-extension.zip
)
echo.

:: ── Done ─────────────────────────────────────────────────────────────────────
echo.
echo ==========================================
echo   Build complete!
echo.
echo   Desktop Installers:
echo     target\!BUILD_TYPE!\bundle\nsis\
echo     target\!BUILD_TYPE!\bundle\msi\
echo.
echo   Browser Extension:
echo     vajra-extension\vajra-extension.zip
echo ==========================================
echo.
pause
