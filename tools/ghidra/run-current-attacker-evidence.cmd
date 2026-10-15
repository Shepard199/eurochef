@echo off
setlocal
if "%~1"=="" exit /b 2
set "GHIDRA=D:\GitHubHome\Ghidra\support\analyzeHeadless.bat"
for %%I in ("%~dp0..\..") do set "ROOT=%%~fI"
set "PROJECT_DIR=%ROOT%\ghidra-projects"
set "SCRIPT_DIR=%ROOT%\tools\ghidra"
set "GHIDRA_JAVA_OPTIONS=-Duser.name=max19"
call "%GHIDRA%" "%PROJECT_DIR%" robots -process Robots.exe -noanalysis -readOnly -scriptPath "%SCRIPT_DIR%" -postScript "%~1"
exit /b %ERRORLEVEL%
