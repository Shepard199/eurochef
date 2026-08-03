@echo off
setlocal

set "RUN_PS=%~dp0..\..\CodexPro-Up\tools\Run-PreferredPowerShell.cmd"
if not exist "%RUN_PS%" (
    echo [ERROR] Preferred PowerShell launcher is missing: %RUN_PS%
    exit /b 2
)

call "%RUN_PS%" "%~dp0tools\Invoke-Cargo.ps1" %*
exit /b %ERRORLEVEL%
