@echo off
setlocal

set "RUN_PS=%~dp0..\..\CodexPro-Up\tools\Run-PreferredPowerShell.cmd"
if not exist "%RUN_PS%" (
    echo [ERROR] Preferred PowerShell launcher is missing: %RUN_PS%
    exit /b 2
)

rem Pass the native Cargo command line out-of-band so PowerShell never mistakes
rem Cargo flags such as -p/--package for parameters of Invoke-Cargo.ps1.
set "EUROCHEF_CARGO_COMMAND_LINE=%*"
call "%RUN_PS%" "%~dp0tools\Invoke-Cargo.ps1"
set "CARGO_RC=%ERRORLEVEL%"
exit /b %CARGO_RC%
