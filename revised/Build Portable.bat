@echo off
setlocal
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0Build.ps1" -Validate
if errorlevel 1 (
    echo Build or validation failed. Review the error above.
    pause
    exit /b 1
)
