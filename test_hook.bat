@echo off
setlocal
cd /d "%~dp0"
"E:\conda\envs\clam_latest\python.exe" test_hook.py
pause
