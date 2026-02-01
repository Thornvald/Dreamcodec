@echo off
title Build Dreamcodec (.exe + Setup)
echo ============================================
echo  Building Dreamcodec - EXE + Installer
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
echo [2/2] Building Tauri with installers (NSIS + MSI)...
call npx tauri build
if %errorlevel% neq 0 (
    echo ERROR: Tauri build failed.
    pause
    exit /b 1
)

echo.
echo ============================================
echo  Build complete!
echo  EXE:   src-tauri\target\release\dreamcodec.exe
echo  NSIS:  src-tauri\target\release\bundle\nsis\
echo  MSI:   src-tauri\target\release\bundle\msi\
echo ============================================
pause
