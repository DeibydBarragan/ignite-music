<#
.SYNOPSIS
    Muestreo de CPU y RAM para Node y Rust en paralelo (Spec S-33).
.DESCRIPTION
    Toma muestras cada 60 segundos sumando el proceso raiz y sus procesos hijos
    (ffmpeg.exe y yt-dlp.exe por ParentProcessId). Escribe medicion.csv.
#>
param(
    [double]$DurationHours = 24,
    [int]$IntervalSeconds = 60,
    [string]$OutputFile = ""
)

if (-not $OutputFile) {
    $pruebaDir = Join-Path $PSScriptRoot '..\..\..\..\pruebas-f7'
    $OutputFile = Join-Path -Path $pruebaDir -ChildPath 'medicion.csv'
}

$ErrorActionPreference = "Continue"

$targetDir = Split-Path -Parent $OutputFile
if ($targetDir -and -not (Test-Path $targetDir)) {
    New-Item -ItemType Directory -Path $targetDir -Force | Out-Null
}

if (-not (Test-Path $OutputFile)) {
    "hora,bot,cpu_pct,ram_mb" | Out-File -FilePath $OutputFile -Encoding utf8
}

$logicalCores = [Environment]::ProcessorCount
if ($logicalCores -lt 1) { $logicalCores = 1 }

function Get-BotRootPid([string]$botType) {
    if ($botType -eq "node") {
        $proc = Get-CimInstance Win32_Process -Filter "Name = 'node.exe'" -ErrorAction SilentlyContinue |
            Where-Object { $_.CommandLine -like "*index.js*" } |
            Select-Object -First 1
        if ($proc) { return $proc.ProcessId }
        $conn = Get-NetTCPConnection -LocalPort 3001 -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($conn) { return $conn.OwningProcess }
    } elseif ($botType -eq "rust") {
        $proc = Get-CimInstance Win32_Process -Filter "Name = 'bot.exe'" -ErrorAction SilentlyContinue |
            Select-Object -First 1
        if ($proc) { return $proc.ProcessId }
        $conn = Get-NetTCPConnection -LocalPort 3002 -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($conn) { return $conn.OwningProcess }
    }
    return $null
}

function Get-DescendantPids([int]$parentPid) {
    $pids = @($parentPid)
    $children = Get-CimInstance Win32_Process -Filter "ParentProcessId = $parentPid" -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -in @('ffmpeg.exe', 'yt-dlp.exe', 'node.exe', 'bot.exe', 'cmd.exe', 'powershell.exe') }
    foreach ($child in $children) {
        $pids += Get-DescendantPids -parentPid $child.ProcessId
    }
    return ($pids | Select-Object -Unique)
}

function Measure-BotProcess([string]$botType, [hashtable]$state) {
    $rootPid = Get-BotRootPid -botType $botType
    if (-not $rootPid) {
        return @{ cpu_pct = 0.0; ram_mb = 0.0 }
    }

    $allPids = Get-DescendantPids -parentPid $rootPid
    $procs = Get-Process -Id $allPids -ErrorAction SilentlyContinue

    if (-not $procs -or $procs.Count -eq 0) {
        return @{ cpu_pct = 0.0; ram_mb = 0.0 }
    }

    $totalWorkingSet = ($procs | Measure-Object -Property WorkingSet64 -Sum).Sum
    $ramMb = [math]::Round($totalWorkingSet / 1MB, 2)

    $now = [DateTime]::UtcNow
    $totalCpuSecs = ($procs | ForEach-Object { $_.TotalProcessorTime.TotalSeconds } | Measure-Object -Sum).Sum

    $cpuPct = 0.0
    if ($state.ContainsKey($botType)) {
        $prevState = $state[$botType]
        $deltaCpu = $totalCpuSecs - $prevState.cpuSecs
        $deltaSecs = ($now - $prevState.time).TotalSeconds
        if ($deltaSecs -gt 0 -and $deltaCpu -ge 0) {
            $rawCpu = ($deltaCpu / ($deltaSecs * $logicalCores)) * 100
            $cpuPct = [math]::Round([math]::Max(0.0, $rawCpu), 2)
        }
    }

    $state[$botType] = @{
        time = $now
        cpuSecs = $totalCpuSecs
    }

    return @{ cpu_pct = $cpuPct; ram_mb = $ramMb }
}

Write-Host "[F7] Iniciando medicion de CPU y RAM ($DurationHours horas, intervalo: ${IntervalSeconds}s)..."
Write-Host "Salida: $OutputFile"

$startTime = [DateTime]::UtcNow
$endTime = $startTime.AddHours($DurationHours)
$state = @{}

while ([DateTime]::UtcNow -lt $endTime) {
    $timestamp = [DateTime]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ssZ")

    foreach ($bot in @("node", "rust")) {
        $m = Measure-BotProcess -botType $bot -state $state
        $line = "$timestamp,$bot,$($m.cpu_pct),$($m.ram_mb)"
        Add-Content -Path $OutputFile -Value $line -Encoding utf8
    }

    Start-Sleep -Seconds $IntervalSeconds
}

Write-Host "[F7] Medicion completada exitosamente."
