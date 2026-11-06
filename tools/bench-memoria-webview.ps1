<#
.SYNOPSIS
  Mide la memoria total de Pliego (app + worker + procesos de WebView2) con distintas opciones del navegador.
.DESCRIPTION
  Criterio de Ander (W01, revisado el 2026-11-06), con un PDF de 100 páginas abierto y en reposo:
    - Memoria propia de Pliego (app + worker): <= 150 MB de conjunto de trabajo (-LimitePropioMb).
    - Cifra del Administrador de tareas (app + worker + todos los procesos de WebView2, conjunto de trabajo): <= 500 MB
      (-LimiteTotalMb). Se mide aparte porque casi todo lo que pesa es el motor de WebView2, que Pliego no controla.
  El script marca cada variante con OK/FALLA contra los dos límites y termina con código 1 si alguna falla.

  Uso (en acr12, tras `. .\tools\acr12-env.ps1`; no hace falta compilar si ya hay un pliego-app.exe):
      .\tools\acr12-turno.ps1 -Flujo w01 -- .\tools\bench-memoria-webview.ps1 -Exe D:\ruta\pliego-app.exe
      .\tools\bench-memoria-webview.ps1 -Exe ... -Variantes base,gpu-interna,combo -Repeticiones 3

  Cada variante arranca la app con una carpeta de datos de WebView2 nueva (WEBVIEW2_USER_DATA_FOLDER) y las opciones
  en WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS, así se prueba sin recompilar. Esa variable sustituye a las opciones por
  defecto de Tauri/wry, por lo que todas las variantes repiten `--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection`
  (Chromium solo hace caso al último `--disable-features`, así que las demás funciones desactivadas van en la misma lista).

  Escribe docs\qa\memoria-webview\<fecha>-<equipo>.json y una tabla en pantalla. No toca nada fuera de su carpeta temporal.
#>
param(
    [string]$Exe,
    [string]$Pdf,
    [string[]]$Variantes,
    [int]$Espera = 25,
    [int]$Repeticiones = 1,
    [int]$LimitePropioMb = 150,
    [int]$LimiteTotalMb = 500,
    [string]$Salida
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot

if (-not $Exe) {
    $target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $repo 'target' }
    $Exe = Join-Path $target 'release\pliego-app.exe'
}
if (-not (Test-Path $Exe)) { throw "No existe ${Exe}: pasa -Exe con la ruta de pliego-app.exe (con pdfium.dll y el worker al lado)." }
if (-not $Pdf) {
    $Pdf = Join-Path $repo 'tests\output\bench-100.pdf'
    if (-not (Test-Path $Pdf)) { python (Join-Path $repo 'tools\gen_bench_pdf.py') --pages 100 --out $Pdf }
}

$base = '--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection'
$sinAislamiento = '--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,IsolateOrigins,site-per-process'
$ligero = '--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,IsolateOrigins,site-per-process,OptimizationHints,Translate,MediaRouter,AutofillServerCommunication,InterestFeedContentSuggestions'
$red = '--disable-background-networking --disable-component-update --disable-sync --disable-breakpad --disable-domain-reliability --no-pings --disk-cache-size=1 --media-cache-size=1'

# Orden = el de la tabla. «base» es lo que lleva hoy la app (opciones por defecto de wry).
$todas = [ordered]@{
    'base'             = $base
    'gpu-interna'      = "$base --in-process-gpu"
    'un-renderer'      = "$base --renderer-process-limit=1"
    'sin-aislamiento'  = "$sinAislamiento --renderer-process-limit=1"
    'ligero'           = "$ligero $red"
    'v8-corto'         = "$base --js-flags=--max-old-space-size=96"
    'v8-lite'          = "$base --js-flags=--lite-mode"
    'sin-gpu'          = "$base --disable-gpu --disable-gpu-compositing"
    'combo'            = "$ligero $red --in-process-gpu --renderer-process-limit=1"
    'combo-v8'         = "$ligero $red --in-process-gpu --renderer-process-limit=1 --js-flags=--max-old-space-size=96"
}
if (-not $Variantes) { $Variantes = @($todas.Keys) }
foreach ($v in $Variantes) { if (-not $todas.Contains($v)) { throw "Variante desconocida: $v (hay: $($todas.Keys -join ', '))" } }

function Get-Tree([int]$rootPid) {
    $all = Get-CimInstance Win32_Process
    $ids = New-Object System.Collections.Generic.HashSet[int]
    [void]$ids.Add($rootPid)
    do {
        $n = $ids.Count
        foreach ($p in $all) { if ($ids.Contains([int]$p.ParentProcessId)) { [void]$ids.Add([int]$p.ProcessId) } }
    } while ($ids.Count -ne $n)
    $all | Where-Object { $ids.Contains([int]$_.ProcessId) }
}

function Get-Kind($proc) {
    $name = $proc.Name.ToLowerInvariant()
    if ($name -eq 'pliego-app.exe') { return 'app' }
    if ($name -like 'pliego-worker*') { return 'worker' }
    if ($name -ne 'msedgewebview2.exe') { return 'otro' }
    $cl = [string]$proc.CommandLine
    if ($cl -match '--type=renderer') { return 'renderer' }
    if ($cl -match '--type=gpu-process') { return 'gpu' }
    if ($cl -match '--type=utility') { return 'utilidad' }
    if ($cl -match '--type=crashpad') { return 'crashpad' }
    if ($cl -match '--type=') { return 'otro-wv2' }
    return 'navegador'
}

function Measure-Once([string]$nombre, [string]$opciones) {
    $datos = Join-Path $env:TEMP ("pliego-wv2-$nombre-" + [guid]::NewGuid().ToString('N').Substring(0, 8))
    $env:WEBVIEW2_USER_DATA_FOLDER = $datos
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = $opciones
    $app = Start-Process -FilePath $Exe -ArgumentList @("`"$Pdf`"") -PassThru
    try {
        Start-Sleep -Seconds $Espera
        if ($app.HasExited) { throw "La app salió (código $($app.ExitCode)) con la variante $nombre" }
        $tree = @(Get-Tree $app.Id)
        $perf = @{}
        foreach ($r in (Get-CimInstance Win32_PerfFormattedData_PerfProc_Process)) { $perf[[int]$r.IDProcess] = $r }
        $porTipo = [ordered]@{}
        $browserCmd = ''
        foreach ($p in $tree) {
            $k = Get-Kind $p
            $r = $perf[[int]$p.ProcessId]
            if (-not $r) { continue }
            if (-not $porTipo.Contains($k)) { $porTipo[$k] = [ordered]@{ n = 0; ws = 0.0; priv = 0.0; commit = 0.0 } }
            $porTipo[$k].n += 1
            $porTipo[$k].ws += $r.WorkingSet / 1MB
            $porTipo[$k].priv += $r.WorkingSetPrivate / 1MB
            $porTipo[$k].commit += $r.PrivateBytes / 1MB
            if ($k -eq 'navegador') { $browserCmd = [string]$p.CommandLine }
        }
        $sum = { param($ks, $campo) ($ks | Where-Object { $porTipo.Contains($_) } | ForEach-Object { $porTipo[$_][$campo] } | Measure-Object -Sum).Sum }
        $wv = @('navegador', 'renderer', 'gpu', 'utilidad', 'crashpad', 'otro-wv2')
        $nucleo = @('app', 'worker')
        $res = [ordered]@{
            variante          = $nombre
            procesos_wv2      = @($tree | Where-Object { $_.Name -eq 'msedgewebview2.exe' }).Count
            app_worker_ws_mb  = [math]::Round((& $sum $nucleo 'ws'), 1)
            app_worker_priv_mb = [math]::Round((& $sum $nucleo 'priv'), 1)
            wv2_ws_mb         = [math]::Round((& $sum $wv 'ws'), 1)
            wv2_priv_mb       = [math]::Round((& $sum $wv 'priv'), 1)
            total_ws_mb       = [math]::Round((& $sum ($nucleo + $wv) 'ws'), 1)
            total_priv_mb     = [math]::Round((& $sum ($nucleo + $wv) 'priv'), 1)
            por_tipo          = $porTipo
            args_navegador    = $browserCmd
        }
        $res['propio_ok'] = ($res.app_worker_ws_mb -le $LimitePropioMb)
        $res['total_ok']  = ($res.total_ws_mb -le $LimiteTotalMb)
        return $res
    }
    finally {
        # Cierra la app y espera a que mueran sus procesos de WebView2 antes de borrar la carpeta de datos.
        Get-Tree $app.Id | Where-Object { $_.ProcessId -ne $PID } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
        Start-Sleep -Seconds 3
        Remove-Item -Recurse -Force $datos -ErrorAction SilentlyContinue
        Remove-Item Env:\WEBVIEW2_USER_DATA_FOLDER, Env:\WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
    }
}

$resultados = @()
foreach ($v in $Variantes) {
    for ($i = 1; $i -le $Repeticiones; $i++) {
        Write-Host "== $v ($i/$Repeticiones)" -ForegroundColor Cyan
        $r = Measure-Once $v $todas[$v]
        $r['repeticion'] = $i
        $resultados += $r
        Write-Host ("   WV2 {0} procesos: {1} MB WS / {2} MB privados · app+worker {3} MB · total {4} MB WS / {5} MB privados" -f `
            $r.procesos_wv2, $r.wv2_ws_mb, $r.wv2_priv_mb, $r.app_worker_ws_mb, $r.total_ws_mb, $r.total_priv_mb)
    }
}

Write-Host ""
$resultados | ForEach-Object { [pscustomobject]$_ } |
    Format-Table variante, repeticion, procesos_wv2, app_worker_ws_mb, propio_ok, wv2_ws_mb, wv2_priv_mb, total_ws_mb, total_ok, total_priv_mb -AutoSize
$fallos = @($resultados | Where-Object { -not $_.propio_ok -or -not $_.total_ok })

if (-not $Salida) {
    $dir = Join-Path $repo 'docs\qa\memoria-webview'
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    $Salida = Join-Path $dir ("{0}-{1}.json" -f (Get-Date -Format 'yyyy-MM-dd-HHmm'), $env:COMPUTERNAME)
}
[ordered]@{
    fecha = (Get-Date -Format 's'); equipo = $env:COMPUTERNAME
    windows = (Get-CimInstance Win32_OperatingSystem).Version
    webview2 = (Get-ItemProperty 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}' -ErrorAction SilentlyContinue).pv
    exe = $Exe; pdf = $Pdf; espera_s = $Espera
    limite_propio_mb = $LimitePropioMb; limite_total_mb = $LimiteTotalMb
    resultados = $resultados
} | ConvertTo-Json -Depth 6 | Set-Content -Path $Salida -Encoding UTF8
Write-Host "Resultado guardado en $Salida"
if ($fallos.Count -gt 0) {
    Write-Host ("FALLA: {0} medida(s) superan {1} MB (propia) o {2} MB (Administrador de tareas)." -f $fallos.Count, $LimitePropioMb, $LimiteTotalMb) -ForegroundColor Red
    exit 1
}
Write-Host ("OK: memoria propia <= {0} MB y total <= {1} MB en todas las medidas." -f $LimitePropioMb, $LimiteTotalMb) -ForegroundColor Green
