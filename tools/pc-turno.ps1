<#
.SYNOPSIS
  Turno único del equipo de pruebas «pc»: envoltorio de tools\pliego-turno.ps1 (mismo mutex
  Global\PliegoAcr12TurnoUnico y cola FIFO que acr12; estado en C:\dev-cache\turnos\turno.txt).

.EXAMPLE
  . .\tools\pc-env.ps1
  .\tools\pc-turno.ps1 -Flujo w10 -- cargo build -p pliego-cli
#>
# Sin param() a propósito: los argumentos pasan tal cual (ver pliego-turno.ps1).
& (Join-Path $PSScriptRoot 'pliego-turno.ps1') @args
exit $LASTEXITCODE
