@echo off
setlocal
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0Build.ps1" -Launch
if errorlevel 1 (
    echo Build failed. Review the error above.
    pause
    exit /b 1
)
