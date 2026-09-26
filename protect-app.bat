@echo off
setlocal
pwsh -NoProfile -File "%~dp0scripts\protect-themida.ps1" %*
exit /b %errorlevel%
