@echo off
setlocal EnableDelayedExpansion
title Vajra — Release Build

echo.
echo  ========================================
echo       VAJRA — Release Build Script       
echo  ========================================
echo.

:: ── Locate MSVC ──
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
pause & exit /b 1
:found_vc
call %VCVARS% >nul 2>&1
echo [OK] MSVC environment loaded.

:: ── Check Rust ──
where cargo >nul 2>&1
if errorlevel 1 (
    echo [ERROR] Rust not found.
    pause & exit /b 1
)
echo [OK] Rust found.

:: ── Check Node ──
where npm >nul 2>&1
if errorlevel 1 (
    echo [ERROR] Node.js not found.
    pause & exit /b 1
)
echo [OK] Node.js found.
echo.

:: ── Build Rust crates (Release) ──
echo [1/3] Building Rust crates (Release)...
set "ATTEMPTS=0"
:build_loop
cargo build --release -j 2 2>&1
if errorlevel 1 (
    set /a ATTEMPTS+=1
    if !ATTEMPTS! lss 10 (
        echo [WARN] Rust build failed due to file lock. Retrying !ATTEMPTS!/10...
        timeout /t 2 >nul
        goto build_loop
    )
    echo [FAIL] Rust release build failed after 10 attempts.
    pause & exit /b 1
)
echo [OK] Rust crates built for release.
echo.
echo [OK] Rust crates built for release.
echo.

:: ── Install npm deps if needed ──
echo [2/3] Checking npm dependencies...
if not exist "vajra-ui-tauri\node_modules" (
    echo Installing npm packages...
    cd vajra-ui-tauri && npm install 2>&1 && cd ..
    if errorlevel 1 ( echo [FAIL] npm install failed. & pause & exit /b 1 )
)
echo [OK] npm dependencies ready.
echo.

:: ── Build Tauri UI (Release) ──
echo [3/3] Building Tauri UI (Release)...
cd vajra-ui-tauri
set "TAURI_ATTEMPTS=0"
:tauri_loop
npm run tauri build 2>&1
if errorlevel 1 (
    set /a TAURI_ATTEMPTS+=1
    if !TAURI_ATTEMPTS! lss 10 (
        echo [WARN] Tauri build failed (likely OS error 32 file lock). Retrying !TAURI_ATTEMPTS!/10...
        timeout /t 2 >nul
        goto tauri_loop
    )
    cd ..
    echo [FAIL] Tauri release build failed after 10 attempts.
    pause & exit /b 1
)
cd ..
echo [OK] Tauri UI release built.
echo.

echo  ========================================
echo   Release Build complete!                
echo   Check: vajra-ui-tauri\src-tauri\target\release\bundle\nsis\
echo  ========================================
echo.
