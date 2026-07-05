@echo off
setlocal EnableDelayedExpansion
title Vajra - Tauri Dev

echo.
echo [1/3] Loading MSVC environment...
set "VCVARS="
for %%P in (
    "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
    "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat"
    "C:\Program Files\Microsoft Visual Studio\2022\Professional\VC\Auxiliary\Build\vcvars64.bat"
    "C:\Program Files\Microsoft Visual Studio\2022\Enterprise\VC\Auxiliary\Build\vcvars64.bat"
    "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvars64.bat"
) do (
    if exist %%P ( set "VCVARS=%%~P" & goto :found_vc )
)
echo [ERROR] Visual Studio 2022 not found. Install VS Build Tools first.
pause & exit /b 1
:found_vc
call "%VCVARS%" >nul 2>&1
echo [OK] MSVC environment loaded.

echo [2/3] Building vajra-daemon...
taskkill /F /IM vajrad-x86_64-pc-windows-msvc.exe >nul 2>&1
taskkill /F /IM vajrad.exe >nul 2>&1
cargo build -p vajra-daemon
if %ERRORLEVEL% neq 0 ( echo [ERROR] Daemon build failed! & pause & exit /b 1 )

echo [OK] Copying daemon sidecar...
if not exist "vajra-ui-tauri\src-tauri\bin" mkdir "vajra-ui-tauri\src-tauri\bin"
set "SRC=%CD%\target\debug\vajrad.exe"
set "DST=%CD%\vajra-ui-tauri\src-tauri\bin\vajrad-x86_64-pc-windows-msvc.exe"
if exist "%SRC%" (
    copy /Y "%SRC%" "%DST%" >nul
    if exist "%DST%" (
        echo [OK] Daemon ready at "%DST%"
    ) else (
        echo [ERROR] Failed to copy daemon binary to "%DST%"
        pause & exit /b 1
    )
) else (
    echo [ERROR] Daemon binary not found at "%SRC%"
    pause & exit /b 1
)

echo [3/3] Starting Tauri dev server...
cd vajra-ui-tauri
npm run tauri dev