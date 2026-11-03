<#
.SYNOPSIS
  Ejecuta una orden pesada (cargo build/clippy/test, tauri build, npm run test:e2e...) en acr12
  respetando el turno: un solo flujo compilando, enlazando o pasando tests pesados a la vez
  (con 2 a la vez acr12, de 16 GB, se quedaba sin memoria y paginaba).

.EXAMPLE
  . .\tools\acr12-env.ps1
  .\tools\acr12-turno.ps1 -Flujo w09 -- cargo test --workspace
  .\tools\acr12-turno.ps1 -Flujo w09 -EsperaMaxMinutos 60 -- cargo build --release -p pliego-app
  powershell -File .\tools\acr12-turno.ps1 -Flujo w09 -- cargo build -p pliego-cli

  Todo lo que va tras `--` es la orden, tal cual. Antes de `--` solo se aceptan -Flujo (obligatorio)
  y -EsperaMaxMinutos; el primer argumento distinto empieza la orden (a veces PowerShell se come el `--`). El script no declara param() a propósito: lee $args a mano para que -p,
  --release, etc. lleguen intactos a la orden, se invoque con `&`, con `.\` o con `powershell -File`.

.NOTES
  Usa un mutex con nombre (Global\PliegoAcr12TurnoUnico). Si una sesión muere con el turno cogido,
  Windows libera el mutex (queda "abandonado") y el siguiente lo toma: no hay turnos huérfanos.
  El estado se escribe en D:\dev-cache\turnos\turno.txt (quién, desde cuándo y PID de la pwsh que lo tiene).
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
if (-not $Flujo) { throw "Falta -Flujo. Uso: acr12-turno.ps1 -Flujo wNN [-EsperaMaxMinutos N] -- <orden>" }
if ($Orden.Count -eq 0) { throw "Falta la orden tras '--'. Uso: acr12-turno.ps1 -Flujo wNN [-EsperaMaxMinutos N] -- <orden>" }
if (-not $env:CARGO_BUILD_JOBS) { $env:CARGO_BUILD_JOBS = '4' }

$estado = 'D:\dev-cache\turnos'
if (Test-Path 'D:\') { New-Item -ItemType Directory -Force -Path $estado | Out-Null }

# Nombre nuevo a propósito: Global\PliegoAcr12Turno1 quedó retenido por un proceso vivo sin fichero de estado.
$mutexes = @(New-Object System.Threading.Mutex($false, 'Global\PliegoAcr12TurnoUnico'))
$mio = $null
$limite = (Get-Date).AddMinutes($EsperaMaxMinutos)
while ($null -eq $mio) {
    for ($i = 0; $i -lt $mutexes.Count -and $null -eq $mio; $i++) {
        try {
            if ($mutexes[$i].WaitOne(0)) { $mio = $i }
        } catch [System.Threading.AbandonedMutexException] {
            $mio = $i   # la sesión anterior murió con el turno: lo heredamos
        }
    }
    if ($null -eq $mio) {
        if ((Get-Date) -gt $limite) {
            Write-Host "[turno] Sin turno en acr12 tras $EsperaMaxMinutos min; no se ha ejecutado nada."
            $mutexes | ForEach-Object { $_.Dispose() }
            exit 75
        }
        $ocupado = Get-ChildItem $estado -Filter 'turno.txt' -ErrorAction SilentlyContinue | ForEach-Object { Get-Content $_.FullName -Raw }
        Write-Host "[turno] acr12 ocupado ($($ocupado -join ' | ')). Reintento en 30 s..."
        Start-Sleep -Seconds 30
    }
}
$codigo = 1
$fichero = Join-Path $estado 'turno.txt'
try {
    Set-Content -Path $fichero -Value ("{0} desde {1:HH:mm}, PID {3} ({2})" -f $Flujo, (Get-Date), ($Orden -join ' '), $PID)
    Write-Host "[turno] $Flujo tiene el turno $($mio + 1). Ejecutando: $($Orden -join ' ')"
    & $Orden[0] @($Orden | Select-Object -Skip 1)
    $codigo = $LASTEXITCODE
} finally {
    Remove-Item -Path $fichero -ErrorAction SilentlyContinue
    $mutexes[$mio].ReleaseMutex()
    $mutexes | ForEach-Object { $_.Dispose() }
}
exit $codigo
