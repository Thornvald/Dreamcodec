@echo off
echo Running app with debug output...
cd /d "%~dp0"
"C:\Users\DevUser\Desktop\Video-Converter\src-tauri\target\release\dreamcodec.exe" > debug-output.txt 2>&1
echo.
echo Debug output saved to debug-output.txt
pause
