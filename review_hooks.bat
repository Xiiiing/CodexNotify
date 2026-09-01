@echo off
setlocal
cd /d "%~dp0"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0review_hooks.ps1"
if errorlevel 1 (
  echo.
  echo Failed to open Codex hook review.
)
pause

