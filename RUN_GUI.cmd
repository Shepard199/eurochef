@echo off
setlocal
set "RUN_PS=%~dp0..\..\CodexPro-Up\tools\Run-PreferredPowerShell.cmd"
if not exist "%RUN_PS%" (
    echo [ERROR] Preferred PowerShell launcher is missing: %RUN_PS%
    exit /b 2
)
call "%RUN_PS%" "%~dp0RUN_GUI.ps1" %*
set "RC=%ERRORLEVEL%"
endlocal & exit /b %RC%
