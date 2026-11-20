@echo off
title Beatra
cd /d "%~dp0"

:loop
if not exist "rust\target\release\bot.exe" (
    echo rust\target\release\bot.exe not found.
    echo Please build the release binary first:
    echo   cd rust ^&^& cargo build --release -p bot
    pause
    exit /b 1
)

rust\target\release\bot.exe
set EXITCODE=%errorlevel%

if "%EXITCODE%"=="0" (
    echo.
    echo Bot stopped.
    pause
    exit /b 0
)

if "%EXITCODE%"=="1" (
    echo.
    echo Bot crashed with exit code 1. Restarting in 5 seconds...
    timeout /t 5 /nobreak >nul
    goto loop
)

echo.
echo Bot exited with code %EXITCODE%.
pause
exit /b %EXITCODE%
