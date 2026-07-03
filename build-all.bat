@echo off
setlocal EnableDelayedExpansion
title Vajra — Build All

echo.
echo  ╔════════════════════════════════════════╗
echo  ║     VAJRA — Full Build Script          ║
echo  ╚════════════════════════════════════════╝
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
set "VCVARS="
for %%P in (
    "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
    "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat"
    "C:\Program Files\Microsoft Visual Studio\2022\Professional\VC\Auxiliary\Build\vcvars64.bat"
    "C:\Program Files\Microsoft Visual Studio\2022\Enterprise\VC\Auxiliary\Build\vcvars64.bat"
    "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvars64.bat"
) do (
    if exist %%P ( set "VCVARS=%%P" & goto :found_vc )
)
echo [ERROR] Visual Studio 2022 not found. Install VS Build Tools first.
echo         https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022
pause & exit /b 1
:found_vc
call %VCVARS% >nul 2>&1
echo [OK] MSVC environment loaded.

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
echo [1/3] Building Rust crates (engine + daemon + CLI) in !BUILD_TYPE! mode...
taskkill /F /IM vajrad.exe >nul 2>&1
cargo build !CARGO_FLAGS! -p vajra-engine -p vajra-protocol -p vajra-daemon -p vajra-cli 2>&1
if errorlevel 1 ( echo [FAIL] Rust build failed. & pause & exit /b 1 )
echo [OK] Rust crates built.
echo.

:: ── Copy sidecar daemon ──────────────────────────────────────────────────────
echo Copying sidecar daemon for Tauri packaging...
copy /Y "target\!BUILD_TYPE!\vajrad.exe" "vajra-ui-tauri\src-tauri\vajrad-x86_64-pc-windows-msvc.exe" >nul 2>&1
if errorlevel 1 ( echo [FAIL] Copying sidecar failed. & pause & exit /b 1 )

:: ── Install npm deps if needed ───────────────────────────────────────────────
echo [2/3] Checking npm dependencies...
if not exist "vajra-ui-tauri\node_modules" (
    echo Installing npm packages...
    cd vajra-ui-tauri && npm install 2>&1 && cd ..
    if errorlevel 1 ( echo [FAIL] npm install failed. & pause & exit /b 1 )
)
echo [OK] npm dependencies ready.
echo.

:: ── Build Tauri UI ───────────────────────────────────────────────────────────
echo [3/3] Building Tauri UI...
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

:: ── Copy daemon into bundle dir ──────────────────────────────────────────────
set "TAURI_BIN=vajra-ui-tauri\src-tauri\target\!BUILD_TYPE!"
if exist "%TAURI_BIN%\vajra-ui-tauri.exe" (
    copy /Y "target\!BUILD_TYPE!\vajrad.exe" "%TAURI_BIN%\vajrad.exe" >nul 2>&1
    copy /Y "target\!BUILD_TYPE!\vajra.exe" "%TAURI_BIN%\vajra.exe" >nul 2>&1
    echo [OK] Copied binaries next to UI binary.
)

echo.
echo  ╔════════════════════════════════════════╗
echo  ║  Build complete!                       ║
echo  ║                                        ║
echo  ║  Run:  vajra-ui-tauri\src-tauri\       ║
echo  ║        target\!BUILD_TYPE!\vajra-ui-tauri.exe ║
echo  ║                                        ║
echo  ║  Or just run:  run-vajra.bat           ║
echo  ╚════════════════════════════════════════╝
echo.
pause
