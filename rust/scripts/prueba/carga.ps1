<#
.SYNOPSIS
    Generador de carga para medicion en paralelo de Node y Rust (Spec S-33).
.DESCRIPTION
    Ejecuta llamadas escalonadas cada 10 minutos durante 24 horas:
    - Minuto 0: 5 busquedas y 1 reproduccion en Node (puerto 3001)
    - Minuto 5: 5 busquedas y 1 reproduccion en Rust (puerto 3002)
#>
param(
    [string]$OperatorDiscordId = $env:DISCORD_OPERATOR_ID,
    [string]$GuildId = $env:TEST_GUILD_ID,
    [string]$ApiSecret = $env:API_SECRET,
    [string]$VoiceChannelId = $env:TEST_VOICE_CHANNEL_ID,
    [int]$NodePort = 3001,
    [int]$RustPort = 3002,
    [double]$DurationHours = 24,
    [string[]]$Queries = @(
        "Rick Astley - Never Gonna Give You Up",
        "Queen - Bohemian Rhapsody",
        "Lofi Hip Hop Beat Chill",
        "Ed Sheeran - Shape of You",
        "The Beatles - Let It Be"
    )
)

$ErrorActionPreference = "Stop"

function Abort([string]$msg) {
    Write-Error $msg
    exit 1
}

if (-not $OperatorDiscordId) {
    Abort "DISCORD_OPERATOR_ID no esta definida. Define $env:DISCORD_OPERATOR_ID=<id de Discord del operador>"
}
if (-not $GuildId) {
    Abort "TEST_GUILD_ID no esta definida. Define $env:TEST_GUILD_ID=<id del servidor de prueba>"
}
if (-not $ApiSecret) {
    # Intentar leer API_SECRET del .env de la raíz, si existe
    $envFile = Join-Path -Path (Split-Path -Parent (Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)))) -ChildPath ".env"
    if (Test-Path $envFile) {
        $line = Get-Content $envFile | Where-Object { $_ -match '^API_SECRET=' } | Select-Object -First 1
        if ($line) {
            $ApiSecret = ($line -replace '^API_SECRET=', '').Trim()
        }
    }
    if (-not $ApiSecret) {
        Abort "API_SECRET no esta definida ni encontrada en .env. Define `$env:API_SECRET=<token de la API> o ejecuta con -ApiSecret."
    }
}

if (-not $VoiceChannelId) {
    $VoiceChannelId = '1012424911110799370'
    Write-Host "[INFO] TEST_VOICE_CHANNEL_ID no definida. Usando canal de pruebas por defecto: $VoiceChannelId"
}

function Invoke-BotWorkload([string]$name, [int]$port, [string]$songToPlay) {
    $headers = @{
        "Authorization" = "Bearer $ApiSecret"
        "X-User-Discord-Id" = $OperatorDiscordId
    }
    $baseUrl = "http://127.0.0.1:$port/api/v1"
    $now = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    Write-Host "[$now] [$name] Iniciando ronda de 5 busquedas en puerto $port..."

    foreach ($q in $Queries) {
        $encoded = [uri]::EscapeDataString($q)
        $url = "$baseUrl/search?q=$encoded&limit=5"
        try {
            $resp = Invoke-RestMethod -Uri $url -Headers $headers -Method GET -TimeoutSec 15
            Write-Host "  -> Busqueda '$q': OK"
        } catch {
            Write-Warning "  -> Error en busqueda '$q' en ${name}: ${_}"
        }
    }

    Write-Host "[$now] [$name] Solicitando reproduccion: '$songToPlay'..."
    $playUrl = "$baseUrl/guilds/$GuildId/play"
    $body = @{ query = $songToPlay; voiceChannelId = $VoiceChannelId } | ConvertTo-Json
    $postHeaders = @{
        "Authorization" = "Bearer $ApiSecret"
        "X-User-Discord-Id" = $OperatorDiscordId
        "Content-Type" = "application/json"
    }

    try {
        $playResp = Invoke-RestMethod -Uri $playUrl -Headers $postHeaders -Method POST -Body $body -TimeoutSec 30
        Write-Host "  -> Reproduccion iniciada: $($playResp.success)"
    } catch {
        Write-Warning "  -> Error en reproduccion en ${name}: ${_}"
    }
}

Write-Host "[F7] Iniciando generador de carga ($DurationHours horas, escalonado 5min)..."
Write-Host "Servidor GuildId: $GuildId | Operador: $OperatorDiscordId"
Write-Host "Canciones configuradas ($($Queries.Count)):"
foreach ($q in $Queries) { Write-Host " - $q" }

$startTime = [DateTime]::UtcNow
$endTime = $startTime.AddHours($DurationHours)
$cycle = 0

while ([DateTime]::UtcNow -lt $endTime) {
    $currentSong = $Queries[$cycle % $Queries.Count]
    $cycle++

    # Minuto 0: Carga en Node
    Invoke-BotWorkload -name "Node" -port $NodePort -songToPlay $currentSong

    # Espera escalonada de 5 minutos (300 s)
    Write-Host "[F7] Esperando 5 minutos para turno de Rust..."
    Start-Sleep -Seconds 300

    if ([DateTime]::UtcNow -ge $endTime) { break }

    # Minuto 5: Carga en Rust
    Invoke-BotWorkload -name "Rust" -port $RustPort -songToPlay $currentSong

    # Espera los 5 minutos restantes del ciclo de 10 minutos (300 s)
    Write-Host "[F7] Esperando 5 minutos para siguiente ciclo de Node..."
    Start-Sleep -Seconds 300
}

Write-Host "[F7] Generacion de carga finalizada exitosamente."
