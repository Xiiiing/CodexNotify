@echo off
setlocal
cd /d "%~dp0"

set "NOTIFIER_PYTHON=E:\conda\envs\clam_latest\python.exe"
set "NOTIFIER_PYTHONW=E:\conda\envs\clam_latest\pythonw.exe"

if not exist "%NOTIFIER_PYTHON%" (
  echo [ERROR] Conda environment clam_latest was not found:
  echo %NOTIFIER_PYTHON%
  pause
  exit /b 1
)

"%NOTIFIER_PYTHON%" -c "import customtkinter, pystray, PIL, cryptography" >nul 2>nul
if errorlevel 1 (
  echo [INFO] Installing Codex Bark Notifier dependencies into conda env clam_latest...
  "%NOTIFIER_PYTHON%" -m pip install -r requirements.txt
  if errorlevel 1 (
    echo [ERROR] Dependency installation failed.
    pause
    exit /b 1
  )
)

if exist "%NOTIFIER_PYTHONW%" (
  start "" "%NOTIFIER_PYTHONW%" app.py
  exit /b 0
)

"%NOTIFIER_PYTHON%" app.py
if errorlevel 1 pause
