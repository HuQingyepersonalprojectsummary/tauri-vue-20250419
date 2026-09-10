@echo off
setlocal
pwsh -NoProfile -File "%~dp0scripts\build-release.ps1" %*
exit /b %errorlevel%
