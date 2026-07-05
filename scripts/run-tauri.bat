@echo off
:: Vajra — Dev mode (hot-reload UI, uses pre-built daemon)
:: The Tauri app will auto-start vajrad from target\debug\vajrad.exe

setlocal

:: Kill any existing daemon so Tauri's sidecar takes ownership
taskkill /F /IM vajrad-x86_64-pc-windows-msvc.exe >nul 2>&1
taskkill /F /IM vajrad.exe >nul 2>&1

:: Load MSVC env
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1
if errorlevel 1 call "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1
if errorlevel 1 call "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1

cd vajra-ui-tauri
cargo build -p vajra-daemon
npm run tauri dev
