@echo off
setlocal
cd /d "%~dp0"

where cargo >nul 2>nul
if errorlevel 1 (
    echo Rust with Cargo is required to build this application.
    echo Install Rust from https://rustup.rs, then double-click this file again.
    pause
    exit /b 1
)

cargo build --release
if errorlevel 1 (
    echo The build failed. Review the message above and try again.
    pause
    exit /b 1
)

start "" "%~dp0target\release\mipoint-dummy-generator.exe"
exit /b 0
