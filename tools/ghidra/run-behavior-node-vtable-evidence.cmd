@echo off
setlocal
set "GHIDRA=D:\GitHubHome\Ghidra\support\analyzeHeadless.bat"
set "PROJECT_DIR=D:\GitHubHome\Robots\Robot_one_click\_tools\eurochef-main_legacy\ghidra-projects"
set "SCRIPT_DIR=D:\GitHubHome\Robots\Robot_one_click\_tools\eurochef-main_legacy\tools\ghidra"
set "OUTPUT=D:\GitHubHome\Robots\Robot_one_click\_tools\eurochef-main_legacy\tools\ghidra\BehaviorNodeVtableEvidence.txt"
set "GHIDRA_JAVA_OPTIONS=-Duser.name=max19"
call "%GHIDRA%" "%PROJECT_DIR%" robots -process Robots.exe -noanalysis -readOnly -scriptPath "%SCRIPT_DIR%" -postScript BehaviorNodeVtableEvidence.java >"%OUTPUT%" 2>&1
exit /b %ERRORLEVEL%
