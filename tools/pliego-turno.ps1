<#
.SYNOPSIS
  Ejecuta una orden pesada (cargo build/clippy/test, tauri build, npm run test:e2e...) en un equipo
  Windows de pruebas (acr12, pc) respetando el turno: un solo flujo compilando, enlazando o pasando tests pesados a la vez
  (con 2 a la vez acr12, de 16 GB, se quedaba sin memoria y paginaba).

.EXAMPLE
  . .\tools\acr12-env.ps1      # en pc: . .\tools\pc-env.ps1
  .\tools\acr12-turno.ps1 -Flujo w09 -- cargo test --workspace   # en pc: .\tools\pc-turno.ps1 (son envoltorios de este)
  .\tools\acr12-turno.ps1 -Flujo w09 -EsperaMaxMinutos 60 -- cargo build --release -p pliego-app
  powershell -File .\tools\acr12-turno.ps1 -Flujo w09 -- cargo build -p pliego-cli

  Todo lo que va tras `--` es la orden, tal cual. Antes de `--` solo se aceptan -Flujo (obligatorio)
  y -EsperaMaxMinutos; el primer argumento distinto empieza la orden (a veces PowerShell se come el `--`). El script no declara param() a propósito: lee $args a mano para que -p,
  --release, etc. lleguen intactos a la orden, se invoque con `&`, con `.\` o con `powershell -File`.

.NOTES
  Usa un mutex con nombre (Global\PliegoAcr12TurnoUnico). Si una sesión muere con el turno cogido,
  Windows libera el mutex (queda "abandonado") y el siguiente lo toma: no hay turnos huérfanos.
  El estado se escribe en <cache>\turnos\turno.txt, con <cache> = $env:PLIEGO_CACHE, o D:\dev-cache si existe D:
  (acr12), o C:\dev-cache (pc) (quién, desde cuándo y PID de la pwsh que lo tiene).
  Si se agota -EsperaMaxMinutos sale con código 75 sin ejecutar nada; si no, con el código de la orden.
#>
$ErrorActionPreference = 'Stop'
$Flujo = $null
$EsperaMaxMinutos = 240
$Orden = @()
$i = 0
while ($i -lt $args.Count) {
    $a = [string]$args[$i]
    if ($a -eq '--') { $Orden = @($args | Select-Object -Skip ($i + 1) | ForEach-Object { [string]$_ }); break }
    elseif ($a -ieq '-Flujo') { $Flujo = [string]$args[$i + 1]; $i += 2 }
    elseif ($a -ieq '-EsperaMaxMinutos') { $EsperaMaxMinutos = [int]$args[$i + 1]; $i += 2 }
    else { $Orden = @($args | Select-Object -Skip $i | ForEach-Object { [string]$_ }); break }  # PowerShell ya se comió el `--`
}
if (-not $Flujo) { throw "Falta -Flujo. Uso: pliego-turno.ps1 -Flujo wNN [-EsperaMaxMinutos N] -- <orden>" }
if ($Orden.Count -eq 0) { throw "Falta la orden tras '--'. Uso: pliego-turno.ps1 -Flujo wNN [-EsperaMaxMinutos N] -- <orden>" }
if (-not $env:CARGO_BUILD_JOBS) { $env:CARGO_BUILD_JOBS = '4' }

$cache = if ($env:PLIEGO_CACHE) { $env:PLIEGO_CACHE } elseif (Test-Path 'D:\') { 'D:\dev-cache' } else { 'C:\dev-cache' }
$estado = Join-Path $cache 'turnos'
New-Item -ItemType Directory -Force -Path $estado | Out-Null

# Nombre nuevo a propósito: Global\PliegoAcr12Turno1 quedó retenido por un proceso vivo sin fichero de estado.
$mutexes = @(New-Object System.Threading.Mutex($false, 'Global\PliegoAcr12TurnoUnico'))
$mio = $null
# Espera bloqueante (no sondeo): Windows atiende a quien espera un mutex aproximadamente por orden de llegada,
# así nadie se queda sin turno porque otro llegó justo cuando se soltaba (aviso de W20).
$ocupado = Get-Content (Join-Path $estado 'turno.txt') -Raw -ErrorAction SilentlyContinue
if ($ocupado) { Write-Host "[turno] $env:COMPUTERNAME ocupado ($($ocupado.Trim())). En cola desde $(Get-Date -Format HH:mm)..." }
try {
    if ($mutexes[0].WaitOne([TimeSpan]::FromMinutes($EsperaMaxMinutos))) { $mio = 0 }
} catch [System.Threading.AbandonedMutexException] {
    $mio = 0   # la sesión anterior murió con el turno: lo heredamos
}
if ($null -eq $mio) {
    Write-Host "[turno] Sin turno en $env:COMPUTERNAME tras $EsperaMaxMinutos min; no se ha ejecutado nada."
    $mutexes | ForEach-Object { $_.Dispose() }
    exit 75
}
$codigo = 1
$fichero = Join-Path $estado 'turno.txt'
try {
    Set-Content -Path $fichero -Value ("{0} desde {1:HH:mm}, PID {3} ({2})" -f $Flujo, (Get-Date), ($Orden -join ' '), $PID)
    Write-Host "[turno] $Flujo tiene el turno $($mio + 1). Ejecutando: $($Orden -join ' ')"
    # Con 'Stop', si quien llama redirige la salida (2>&1, | Out-File), PowerShell 5.1 convierte cada línea de
    # stderr de la orden (p. ej. "Updating crates.io index" de cargo) en un error que la corta. Vale el código de salida.
    $ErrorActionPreference = 'Continue'
    & $Orden[0] @($Orden | Select-Object -Skip 1)
    $codigo = $LASTEXITCODE
} finally {
    Remove-Item -Path $fichero -ErrorAction SilentlyContinue
    $mutexes[$mio].ReleaseMutex()
    $mutexes | ForEach-Object { $_.Dispose() }
}
exit $codigo
