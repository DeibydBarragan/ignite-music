<#
.SYNOPSIS
    Genera el resumen de medicion y comparativa de paridad (Spec S-33).
.DESCRIPTION
    Lee medicion.csv y los logs de Node y Rust, calcula percentiles y tasas por hora,
    y genera resumen.md con la tabla de evaluacion.
#>
param(
    [string]$CsvPath = "",
    [string]$NodeLog = "",
    [string]$RustLog = "",
    [string]$OutputPath = ""
)

$pruebaDir = Join-Path $PSScriptRoot '..\..\..\..\pruebas-f7'
if (-not (Test-Path $pruebaDir)) {
    New-Item -ItemType Directory -Path $pruebaDir -Force | Out-Null
}
if (-not $CsvPath) { $CsvPath = Join-Path -Path $pruebaDir -ChildPath 'medicion.csv' }
if (-not $NodeLog) { $NodeLog = Join-Path -Path $pruebaDir -ChildPath 'logs\node.log' }
if (-not $RustLog) { $RustLog = Join-Path -Path $pruebaDir -ChildPath 'logs\rust.log' }
if (-not $OutputPath) { $OutputPath = Join-Path -Path $pruebaDir -ChildPath 'resumen.md' }

$ErrorActionPreference = "Continue"

function Get-Percentile([object[]]$list, [double]$p) {
    if ($null -eq $list) { return 0.0 }
    $arr = [double[]]@($list | ForEach-Object { [double]$_ })
    if ($arr.Count -eq 0) { return 0.0 }
    $sorted = $arr | Sort-Object
    $idx = [math]::Floor($p * ($sorted.Count - 1))
    return [math]::Round($sorted[[int]$idx], 2)
}

function Get-Max([double[]]$list) {
    if (-not $list -or $list.Count -eq 0) { return 0.0 }
    return [math]::Round(($list | Measure-Object -Maximum).Maximum, 2)
}

# 1. Parse CSV
$nodeCpu = @(); $nodeRam = @()
$rustCpu = @(); $rustRam = @()
$totalHours = 1.0

if (Test-Path $CsvPath) {
    $rows = Import-Csv -Path $CsvPath
    if ($rows.Count -gt 0) {
        $nodeRows = $rows | Where-Object { $_.bot -eq "node" }
        $rustRows = $rows | Where-Object { $_.bot -eq "rust" }

        $nodeCpu = [double[]]($nodeRows | ForEach-Object { [double]$_.cpu_pct })
        $nodeRam = [double[]]($nodeRows | ForEach-Object { [double]$_.ram_mb })
        $rustCpu = [double[]]($rustRows | ForEach-Object { [double]$_.cpu_pct })
        $rustRam = [double[]]($rustRows | ForEach-Object { [double]$_.ram_mb })

        try {
            $tMin = [DateTime]::Parse($rows[0].hora)
            $tMax = [DateTime]::Parse($rows[-1].hora)
            $diff = ($tMax - $tMin).TotalHours
            if ($diff -gt 0.05) { $totalHours = $diff }
        } catch { }
    }
}

# 2. Parse Logs
function Parse-BotLog([string]$path, [double]$hours) {
    $cuts = 0; $f403 = 0; $botCheck = 0; $unplayable = 0
    $groqLatencies = @(); $searchTimes = @()

    if (Test-Path $path) {
        $lines = Get-Content -Path $path
        foreach ($line in $lines) {
            if ($line -match "finished prematurely at") { $cuts++ }
            if ($line -match "403" -or $line -match "Forbidden") {
                if ($line -match '"level":"(error|warn)"' -or $line -match '\[(error|warn)\]' -or $line -match 'WARN|ERROR') {
                    $f403++
                }
            }
            if ($line -match "Sign in to confirm") { $botCheck++ }
            if ($line -match '"level":"error"' -or $line -match '\[error\]') {
                if ($line -match '\[.+\] .+:' -or $line -match 'track_could_not_play' -or $line -match 'Unknown playback error') {
                    $unplayable++
                }
            }
            if ($line -match 'Voice input transcribed by Groq Whisper') {
                if ($line -match 'elapsed[M_]s[":= ]+([0-9.]+)') {
                    $groqLatencies += [double]$matches[1]
                }
            }
            if ($line -match 'Playing "' -and $line -match 'found in ([0-9.]+) ?s') {
                $searchTimes += [double]$matches[1]
            }
        }
    }

    return @{
        cuts_per_h = [math]::Round($cuts / $hours, 2)
        f403_per_h = [math]::Round($f403 / $hours, 2)
        botcheck_per_h = [math]::Round($botCheck / $hours, 2)
        unplayable_per_h = [math]::Round($unplayable / $hours, 2)
        groq_med = (Get-Percentile $groqLatencies 0.50)
        groq_p95 = (Get-Percentile $groqLatencies 0.95)
        search_med = (Get-Percentile $searchTimes 0.50)
        search_p95 = (Get-Percentile $searchTimes 0.95)
    }
}

$nodeLogStats = Parse-BotLog -path $NodeLog -hours $totalHours
$rustLogStats = Parse-BotLog -path $RustLog -hours $totalHours

$nodeCpuMed = Get-Percentile $nodeCpu 0.50; $nodeCpuP95 = Get-Percentile $nodeCpu 0.95
$rustCpuMed = Get-Percentile $rustCpu 0.50; $rustCpuP95 = Get-Percentile $rustCpu 0.95
$nodeRamMed = Get-Percentile $nodeRam 0.50; $nodeRamMax = Get-Max $nodeRam
$rustRamMed = Get-Percentile $rustRam 0.50; $rustRamMax = Get-Max $rustRam

$hFormatted = [math]::Round($totalHours, 2)
$report = @"
# Resumen de Medición en Paralelo: Node vs Rust (Spec S-33)

- **Duración de la prueba**: $hFormatted horas
- **Muestras registradas en CSV**: $(if (Test-Path $CsvPath) { (Get-Content $CsvPath).Count - 1 } else { 0 })
- **Fecha de generación**: $([DateTime]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ssZ"))

## Tabla Comparativa de Métricas

| Métrica | Cálculo | Node.js (ref) | Rust | Diferencia | Criterio |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **CPU Mediana** | Mediana `cpu_pct` | ${nodeCpuMed}% | ${rustCpuMed}% | $([math]::Round($rustCpuMed - $nodeCpuMed, 2))% | Menor o igual |
| **CPU p95** | Percentil 95 `cpu_pct` | ${nodeCpuP95}% | ${rustCpuP95}% | $([math]::Round($rustCpuP95 - $nodeCpuP95, 2))% | Menor o igual |
| **RAM Mediana** | Mediana `ram_mb` | ${nodeRamMed} MB | ${rustRamMed} MB | $([math]::Round($rustRamMed - $nodeRamMed, 2)) MB | Menor o igual |
| **RAM Máximo** | Máximo `ram_mb` | ${nodeRamMax} MB | ${rustRamMax} MB | $([math]::Round($rustRamMax - $nodeRamMax, 2)) MB | Menor o igual |
| **Cortes** | Eventos/hora | $($nodeLogStats.cuts_per_h) /h | $($rustLogStats.cuts_per_h) /h | $([math]::Round($rustLogStats.cuts_per_h - $nodeLogStats.cuts_per_h, 2)) /h | Sin margen (Rust <= Node) |
| **403 / Forbidden** | Eventos/hora | $($nodeLogStats.f403_per_h) /h | $($rustLogStats.f403_per_h) /h | $([math]::Round($rustLogStats.f403_per_h - $nodeLogStats.f403_per_h, 2)) /h | Sin margen (Rust <= Node) |
| **Bot-check** | Eventos/hora | $($nodeLogStats.botcheck_per_h) /h | $($rustLogStats.botcheck_per_h) /h | $([math]::Round($rustLogStats.botcheck_per_h - $nodeLogStats.botcheck_per_h, 2)) /h | Sin margen (Rust <= Node) |
| **Pistas sin sonar**| Eventos/hora | $($nodeLogStats.unplayable_per_h) /h | $($rustLogStats.unplayable_per_h) /h | $([math]::Round($rustLogStats.unplayable_per_h - $nodeLogStats.unplayable_per_h, 2)) /h | Sin margen (Rust <= Node) |
| **Groq STT Mediana**| Mediana `elapsedMs` | $($nodeLogStats.groq_med) ms | $($rustLogStats.groq_med) ms | $([math]::Round($rustLogStats.groq_med - $nodeLogStats.groq_med, 2)) ms | Menor o igual |
| **Groq STT p95** | Percentil 95 `elapsedMs` | $($nodeLogStats.groq_p95) ms | $($rustLogStats.groq_p95) ms | $([math]::Round($rustLogStats.groq_p95 - $nodeLogStats.groq_p95, 2)) ms | Menor o igual |
| **yt-dlp Búsqueda** | Mediana `found in` | $($nodeLogStats.search_med) s | $($rustLogStats.search_med) s | $([math]::Round($rustLogStats.search_med - $nodeLogStats.search_med, 2)) s | Menor o igual |
| **yt-dlp Búsqueda p95** | p95 `found in` | $($nodeLogStats.search_p95) s | $($rustLogStats.search_p95) s | $([math]::Round($rustLogStats.search_p95 - $nodeLogStats.search_p95, 2)) s | Menor o igual |

## Evaluación de Criterios de Aceptación (S-33)

- **Cortes prematuros**: $(if ($rustLogStats.cuts_per_h -le $nodeLogStats.cuts_per_h) { "✅ CUMPLE (Rust <= Node)" } else { "❌ NO CUMPLE" })
- **Errores 403**: $(if ($rustLogStats.f403_per_h -le $nodeLogStats.f403_per_h) { "✅ CUMPLE (Rust <= Node)" } else { "❌ NO CUMPLE" })
- **Bot-check (Sign in to confirm)**: $(if ($rustLogStats.botcheck_per_h -le $nodeLogStats.botcheck_per_h) { "✅ CUMPLE (Rust <= Node)" } else { "❌ NO CUMPLE" })
- **Pistas que no suenan**: $(if ($rustLogStats.unplayable_per_h -le $nodeLogStats.unplayable_per_h) { "✅ CUMPLE (Rust <= Node)" } else { "❌ NO CUMPLE" })
- **Decisión pot.js**: $(if ($rustLogStats.f403_per_h -le $nodeLogStats.f403_per_h -and $rustLogStats.botcheck_per_h -le $nodeLogStats.botcheck_per_h) { "Aprobada retirada de pot.js (F4 a). Rust sin pot no supera a Node con pot." } else { "Reabrir decision y evaluar Plan B (relay S-15)." })

"@

$outDir = Split-Path -Parent $OutputPath
if ($outDir -and -not (Test-Path $outDir)) {
    New-Item -ItemType Directory -Path $outDir -Force | Out-Null
}

$report | Out-File -FilePath $OutputPath -Encoding utf8
Write-Host $report
Write-Host "[F7] Reporte generado en: $OutputPath"
