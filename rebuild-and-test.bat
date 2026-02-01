@echo off
echo Closing any running instances...
taskkill /IM dreamcodec.exe /F >nul 2>&1

echo Building release version with debug logging...
cd /d "%~dp0"
call npm run tauri build

echo.
echo Build complete! Running the app to test...
echo Check the console output for FFmpeg detection messages.
pause

start "" "%~dp0src-tauri\target\release\dreamcodec.exe"
