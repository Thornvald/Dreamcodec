@echo off
title Build Dreamcodec (.exe only)
echo ============================================
echo  Building Dreamcodec - EXE Only
echo ============================================
echo.

cd /d "%~dp0"

echo [1/2] Building frontend...
call npm run build
if %errorlevel% neq 0 (
    echo ERROR: Frontend build failed.
    pause
    exit /b 1
)

echo.
echo [2/2] Building Tauri (exe only, no installer)...
call npx tauri build --no-bundle
if %errorlevel% neq 0 (
    echo ERROR: Tauri build failed.
    pause
    exit /b 1
)

echo.
echo ============================================
echo  Build complete!
echo  EXE: src-tauri\target\release\dreamcodec.exe
echo ============================================
pause
