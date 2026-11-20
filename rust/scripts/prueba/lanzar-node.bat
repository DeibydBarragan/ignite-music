@echo off
title Ignite Test - Node.js (F7)
cd /d "%~dp0\..\..\.."

if not exist "..\pruebas-f7\logs" mkdir "..\pruebas-f7\logs"
if not exist "..\pruebas-f7\node-data" mkdir "..\pruebas-f7\node-data"

if not defined TEST_DISCORD_TOKEN_NODE (
    echo [AVISO] TEST_DISCORD_TOKEN_NODE no esta definida. Usando DISCORD_TOKEN actual.
) else (
    set "DISCORD_TOKEN=%TEST_DISCORD_TOKEN_NODE%"
)

if defined TEST_GUILD_ID (
    set "GUILD_ID=%TEST_GUILD_ID%"
)

set "CLIENT_ID="
set "DATA_DIR=..\pruebas-f7\node-data"
set "BIN_DIR=bin"
set "API_PORT=3001"
set "AUTO_TUNNEL=false"
set "PUBLIC_API_URL="
set "DEBUG=true"

echo [F7] Iniciando Node.js de prueba en puerto 3001...
echo Los logs se guardan en ..\pruebas-f7\logs\node.log

powershell -NoProfile -Command "node index.js 2>&1 | Tee-Object -FilePath '..\pruebas-f7\logs\node.log'"
