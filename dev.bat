@echo off
title Dreamcodec - Dev Mode
echo ============================================
echo  Starting Dreamcodec in Dev Mode
echo ============================================
echo.

cd /d "%~dp0"

call npx tauri dev
