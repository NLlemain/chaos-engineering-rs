@echo off
echo ========================================
echo  Running Chaos Test as Administrator
echo ========================================
echo.

REM Check if running as admin
net session >nul 2>&1
if %errorLevel% == 0 (
    echo [OK] Running as Administrator
    echo.
    cd /d "%~dp0"
    powershell.exe -ExecutionPolicy Bypass -File ".\scripts\stress_test.ps1"
) else (
    echo [!] Not running as Administrator!
    echo.
    echo To run with full network chaos capabilities:
    echo   Right-click this file and select "Run as administrator"
    echo.
    pause
    exit /b 1
)

echo.
echo Test Complete!
pause
