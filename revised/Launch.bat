@echo off
setlocal
if exist "%~dp0dist\portable\mipoint-dummy-generator.exe" (
    start "" "%~dp0dist\portable\mipoint-dummy-generator.exe"
    exit /b 0
)
call "%~dp0Build and Run.bat"
