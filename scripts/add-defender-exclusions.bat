:: Run this as Administrator (right-click → Run as administrator)
@echo off
echo Adding Windows Defender exclusions for Vajra Rust build...
powershell -Command "Add-MpPreference -ExclusionPath 'D:\Project\Project-Vajra\target'"
powershell -Command "Add-MpPreference -ExclusionPath 'D:\Project\Project-Vajra'"
powershell -Command "Add-MpPreference -ExclusionProcess 'cargo.exe'"
powershell -Command "Add-MpPreference -ExclusionProcess 'rustc.exe'"
powershell -Command "Add-MpPreference -ExclusionProcess 'link.exe'"
powershell -Command "Add-MpPreference -ExclusionProcess 'cl.exe'"
echo.
echo Done! Now run build-rust.bat to compile the engine.
pause
