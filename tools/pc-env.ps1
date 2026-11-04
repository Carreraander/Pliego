<#
.SYNOPSIS
  Entorno de compilación de Pliego en el equipo de pruebas «pc» (DESKTOP-TBC7MHI), SOLO para la terminal actual.

.DESCRIPTION
  Uso (con punto):   . .\tools\pc-env.ps1
  Envoltorio de tools\pliego-env.ps1. pc tiene un solo disco (C:), así que todo va a C:\dev-cache
  (target por clon, TEMP, npm, Playwright, vcpkg) y las herramientas a C:\dev-tools. Ver docs/qa/pc.md.
#>
. (Join-Path $PSScriptRoot 'pliego-env.ps1')
