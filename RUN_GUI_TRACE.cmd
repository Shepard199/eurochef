@echo off
setlocal
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0RUN_GUI.ps1" -Trace %*
set "RC=%ERRORLEVEL%"
endlocal & exit /b %RC%
