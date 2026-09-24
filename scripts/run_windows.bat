@echo off
rem steM. — Windows Launcher Script
setlocal

cd /d "%~dp0\.."

if exist ".venv\Scripts\python.exe" (
    set "PYTHON_EXE=.venv\Scripts\python.exe"
) else (
    set "PYTHON_EXE=python.exe"
)

set "PYTHONPATH=%CD%;%PYTHONPATH%"

"%PYTHON_EXE%" -m stem.app %*
if errorlevel 1 (
    echo.
    echo Application exited with error code %errorlevel%.
    pause
)
