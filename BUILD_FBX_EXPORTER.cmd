@echo off
setlocal
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0tools\fbx-exporter\Build-FbxExporter.ps1" %*
exit /b %ERRORLEVEL%
