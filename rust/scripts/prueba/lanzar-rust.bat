@echo off
title Ignite Test - Rust (F7)
cd /d "%~dp0\..\..\.."

if not exist "..\pruebas-f7\logs" mkdir "..\pruebas-f7\logs"
if not exist "..\pruebas-f7\rust-data" mkdir "..\pruebas-f7\rust-data"
if not exist "..\pruebas-f7\rust-bin" mkdir "..\pruebas-f7\rust-bin"

if not defined TEST_DISCORD_TOKEN_RUST (
    echo [AVISO] TEST_DISCORD_TOKEN_RUST no esta definida. Usando DISCORD_TOKEN actual.
) else (
    set "DISCORD_TOKEN=%TEST_DISCORD_TOKEN_RUST%"
)

if defined TEST_GUILD_ID (
    set "GUILD_ID=%TEST_GUILD_ID%"
)

set "CLIENT_ID="
set "DATA_DIR=..\pruebas-f7\rust-data"
set "BIN_DIR=..\pruebas-f7\rust-bin"
set "API_PORT=3002"
if not defined AUTO_TUNNEL set "AUTO_TUNNEL=false"
if not defined COOKIES_FILE set "COOKIES_FILE=cookies.txt"
set "PUBLIC_API_URL="
set "DEBUG=true"

if not exist "rust\target\release\bot.exe" (
    echo [AVISO] rust\target\release\bot.exe no existe. Buscando en target\debug\bot.exe...
    if exist "rust\target\debug\bot.exe" (
        set "BOT_EXE=rust\target\debug\bot.exe"
    ) else (
        echo [ERROR] No se encontro el ejecutable bot.exe compilado.
        echo Compile primero con: cd rust ^&^& cargo build --release -p bot
        pause
        exit /b 1
    )
) else (
    set "BOT_EXE=rust\target\release\bot.exe"
)

echo [F7] Iniciando Rust de prueba en puerto 3002...
echo Ejecutable: %BOT_EXE%
echo Los logs se guardan en ..\pruebas-f7\logs\rust.log

powershell -NoProfile -Command "& '%BOT_EXE%' 2>&1 | Tee-Object -FilePath '..\pruebas-f7\logs\rust.log'"
