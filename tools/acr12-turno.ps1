<#
.SYNOPSIS
  Ejecuta una orden pesada (cargo build/clippy/test, tauri build, npm run test:e2e...) en acr12
  respetando el turno: como mucho 2 flujos compilando a la vez en la máquina.

.EXAMPLE
  . .\tools\acr12-env.ps1
  .\tools\acr12-turno.ps1 -Flujo w09 -- cargo test --workspace

.NOTES
  Usa dos mutex con nombre (Global\PliegoAcr12Turno1 y ...2). Si una sesión muere con el turno cogido,
  Windows libera el mutex (queda "abandonado") y el siguiente lo toma: no hay turnos huérfanos.
  El estado se escribe en D:\dev-cache\turnos\turno<N>.txt (quién y desde cuándo) para poder consultarlo.
#>
param(
    [Parameter(Mandatory = $true)][string]$Flujo,
    [int]$EsperaMaxMinutos = 240,
    [Parameter(Mandatory = $true, ValueFromRemainingArguments = $true)][string[]]$Orden
)
$ErrorActionPreference = 'Stop'
if ($Orden.Count -gt 0 -and $Orden[0] -eq '--') { $Orden = $Orden[1..($Orden.Count - 1)] }
if (-not $env:CARGO_BUILD_JOBS) { $env:CARGO_BUILD_JOBS = '4' }

$estado = 'D:\dev-cache\turnos'
if (Test-Path 'D:\') { New-Item -ItemType Directory -Force -Path $estado | Out-Null }

$mutexes = 1..2 | ForEach-Object { New-Object System.Threading.Mutex($false, "Global\PliegoAcr12Turno$_") }
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
        if ((Get-Date) -gt $limite) { throw "Sin turno en acr12 tras $EsperaMaxMinutos min" }
        $ocupado = Get-ChildItem $estado -Filter 'turno*.txt' -ErrorAction SilentlyContinue | ForEach-Object { Get-Content $_.FullName -Raw }
        Write-Host "[turno] acr12 ocupado ($($ocupado -join ' | ')). Reintento en 30 s..."
        Start-Sleep -Seconds 30
    }
}
$codigo = 1
$fichero = Join-Path $estado ("turno{0}.txt" -f ($mio + 1))
try {
    Set-Content -Path $fichero -Value ("{0} desde {1:HH:mm} ({2})" -f $Flujo, (Get-Date), ($Orden -join ' '))
    Write-Host "[turno] $Flujo tiene el turno $($mio + 1). Ejecutando: $($Orden -join ' ')"
    & $Orden[0] @($Orden | Select-Object -Skip 1)
    $codigo = $LASTEXITCODE
} finally {
    Remove-Item -Path $fichero -ErrorAction SilentlyContinue
    $mutexes[$mio].ReleaseMutex()
    $mutexes | ForEach-Object { $_.Dispose() }
}
exit $codigo
