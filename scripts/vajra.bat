@echo off
:: Vajra — Single-click launcher
:: Starts the Tauri UI which auto-manages the daemon.
:: Also registers the vajra:// URL protocol handler (once) for browser auto-start.

setlocal

set "TAURI_EXE=target\debug\vajra-ui-tauri.exe"
set "RELEASE_EXE=target\release\vajra-ui-tauri.exe"

:: Determine which exe to use
set "EXE_PATH="
if exist "%~dp0%RELEASE_EXE%" set "EXE_PATH=%~dp0%RELEASE_EXE%"
if exist "%~dp0%TAURI_EXE%"   set "EXE_PATH=%~dp0%TAURI_EXE%"

if "%EXE_PATH%"=="" (
    echo [ERROR] Vajra has not been built yet.
    echo Please run:  build-all.bat
    pause
    exit /b 1
)

:: ── Register vajra:// URL protocol handler (idempotent — safe to run every launch) ──
:: This lets the browser extension auto-start Vajra by opening vajra://start
reg add "HKCU\Software\Classes\vajra"                           /ve /d "URL:Vajra Protocol"  /f >nul 2>&1
reg add "HKCU\Software\Classes\vajra"                           /v "URL Protocol" /d "" /f   >nul 2>&1
reg add "HKCU\Software\Classes\vajra\DefaultIcon"               /ve /d "\"%EXE_PATH%\",0"    /f >nul 2>&1
reg add "HKCU\Software\Classes\vajra\shell\open\command"        /ve /d "\"%EXE_PATH%\" \"%%1\""     /f >nul 2>&1

:: ── Launch Vajra ──────────────────────────────────────────────────────────────
start "" "%EXE_PATH%"
exit /b 0
