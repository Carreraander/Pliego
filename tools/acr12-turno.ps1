<#
.SYNOPSIS
  Turno único de acr12: envoltorio de tools\pliego-turno.ps1 (mutex Global\PliegoAcr12TurnoUnico, FIFO,
  estado en D:\dev-cache\turnos\turno.txt, código 75 si se agota la espera).

.EXAMPLE
  . .\tools\acr12-env.ps1
  .\tools\acr12-turno.ps1 -Flujo w09 -- cargo test --workspace
  .\tools\acr12-turno.ps1 -Flujo w09 -EsperaMaxMinutos 60 -- cargo build --release -p pliego-app
  powershell -File .\tools\acr12-turno.ps1 -Flujo w09 -- cargo build -p pliego-cli
#>
# Sin param() a propósito: los argumentos pasan tal cual (ver pliego-turno.ps1).
& (Join-Path $PSScriptRoot 'pliego-turno.ps1') @args
exit $LASTEXITCODE
