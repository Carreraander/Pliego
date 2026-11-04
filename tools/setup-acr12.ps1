<#
.SYNOPSIS
  Prepara (o verifica) las herramientas de desarrollo de Pliego en el equipo de pruebas acr12.

.DESCRIPTION
  Envoltorio de tools\setup-windows.ps1 con los valores de acr12: disco D:, clones en D:\dev y LLVM 19 en D:\LLVM19.
  Idempotente. Uso:
    powershell -ExecutionPolicy Bypass -File tools\setup-acr12.ps1            # instala lo que falte
    powershell -ExecutionPolicy Bypass -File tools\setup-acr12.ps1 -CheckOnly # solo verifica
    powershell -ExecutionPolicy Bypass -File tools\setup-acr12.ps1 -Report C:\ruta\informe.md
  Código de salida: 0 si todo está disponible al final, 1 si falta algo.
#>
[CmdletBinding()]
param(
    [switch]$CheckOnly,
    [string]$DevRoot = $(if (Test-Path 'D:\') { 'D:\dev' } else { Join-Path $env:USERPROFILE 'dev' }),
    [string]$Report
)
$a = @{ DevRoot = $DevRoot; Llvm19Dir = 'D:\LLVM19' }
if (Test-Path 'D:\') { $a.Disco = 'D:' }
if ($CheckOnly) { $a.CheckOnly = $true }
if ($Report) { $a.Report = $Report }
& (Join-Path $PSScriptRoot 'setup-windows.ps1') @a
exit $LASTEXITCODE
