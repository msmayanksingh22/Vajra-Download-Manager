@echo off
setlocal EnableDelayedExpansion

echo [INFO] Initializing Visual Studio environment...

set "VCVARS="
if exist "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvars64.bat" set "VCVARS=C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvars64.bat"
if exist "C:\Program Files (x86)\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat" set "VCVARS=C:\Program Files (x86)\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat"
if exist "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" set "VCVARS=C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"

if not "%VCVARS%"=="" (
    echo [INFO] Found vcvars64.bat at "%VCVARS%"
    call "%VCVARS%"
) else (
    echo [WARNING] Could not find vcvars64.bat. Build may fail.
)

echo [INFO] Navigating to vajra-ui-tauri...
cd /d "d:\Project\Project-Vajra\vajra-ui-tauri"

echo [INFO] Starting Tauri Build...
set "RETRY_COUNT=0"

:BuildLoop
call npm run tauri build
if %errorlevel% neq 0 (
    set /a RETRY_COUNT+=1
    echo [WARNING] Build failed - possibly due to OS Error 32 or Antivirus file lock.
    if !RETRY_COUNT! lss 4 (
        echo [INFO] Retrying build (Attempt !RETRY_COUNT! of 3)...
        timeout /t 2 /nobreak >nul
        goto BuildLoop
    ) else (
        echo.
        echo [ERROR] Build failed after 3 retries. Check the logs above.
        pause
        exit /b %errorlevel%
    )
)

echo.
echo [SUCCESS] Build completed! Installer is located in:
echo d:\Project\Project-Vajra\vajra-ui-tauri\src-tauri\target\release\bundle\msi\

echo [INFO] Build Process Completed.
pause
