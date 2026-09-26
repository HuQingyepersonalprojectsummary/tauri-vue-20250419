@echo off
setlocal
pwsh -NoProfile -File "%~dp0scripts\build-commercial-release.ps1" %*
exit /b %errorlevel%
