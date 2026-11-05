<#
.SYNOPSIS
  Descarga pdfium.dll (bblanchon/pdfium-binaries) de la versión fijada en pdfium.lock.json y verifica su SHA-256.

.DESCRIPTION
  Sin -Primera exige que version y sha256 estén fijados (si no, falla). Con -Primera descarga la versión de -Version,
  calcula el hash y lo imprime para copiarlo a pdfium.lock.json (revisión humana en el PR).
  Salida: <OutDir>\pdfium.dll, <OutDir>\LICENSE.pdfium y <OutDir>\licenses\ (avisos de terceros del paquete).

.EXAMPLE
  . .\tools\acr12-env.ps1
  .\tools\pdfium\fetch-pdfium.ps1 -Primera -Version chromium/7543 -OutDir D:\dev-cache\pdfium
  .\tools\pdfium\fetch-pdfium.ps1 -OutDir D:\dev-cache\pdfium
#>
[CmdletBinding()]
param(
  [string]$OutDir = (Join-Path $PSScriptRoot '..\..\target\pdfium'),
  [string]$Version,
  [switch]$Primera
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$lockPath = Join-Path $PSScriptRoot 'pdfium.lock.json'
$lock = Get-Content $lockPath -Raw | ConvertFrom-Json
if ($Primera) {
  if (-not $Version) { throw 'Con -Primera indica -Version (p. ej. chromium/7543).' }
} else {
  if (-not $lock.version -or -not $lock.sha256) { throw 'pdfium.lock.json sin versión/hash fijados: ejecuta una vez con -Primera.' }
  $Version = $lock.version
}
$url = "$($lock.fuente)/download/$Version/$($lock.asset)"
New-Item -ItemType Directory -Force $OutDir | Out-Null
$tgz = Join-Path $OutDir $lock.asset
Write-Host "Descargando $url"
Invoke-WebRequest -Uri $url -OutFile $tgz -UseBasicParsing
$hash = (Get-FileHash $tgz -Algorithm SHA256).Hash.ToLower()
if ($Primera) {
  Write-Host "SHA-256 de $($lock.asset) ($Version): $hash"
  Write-Host 'Copia version y sha256 a tools\pdfium\pdfium.lock.json.'
} elseif ($hash -ne $lock.sha256.ToLower()) {
  Remove-Item $tgz -Force
  throw "Hash distinto para $($lock.asset): esperado $($lock.sha256), obtenido $hash. Descarga borrada."
}
$stage = Join-Path $OutDir 'extract'
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory $stage | Out-Null
tar -xzf $tgz -C $stage
if ($LASTEXITCODE -ne 0) { throw 'tar no pudo extraer el paquete.' }
Copy-Item (Join-Path $stage 'bin\pdfium.dll') (Join-Path $OutDir 'pdfium.dll') -Force
Copy-Item (Join-Path $stage 'LICENSE') (Join-Path $OutDir 'LICENSE.pdfium') -Force
if (Test-Path (Join-Path $stage 'licenses')) { Copy-Item (Join-Path $stage 'licenses') (Join-Path $OutDir 'licenses') -Recurse -Force }
$dll = Get-Item (Join-Path $OutDir 'pdfium.dll')
$dllHash = (Get-FileHash $dll.FullName -Algorithm SHA256).Hash.ToLower()
if ($Primera) {
  Write-Host "SHA-256 de pdfium.dll ($Version): $dllHash"
} elseif ($lock.PSObject.Properties['dll_sha256'] -and $lock.dll_sha256 -and $dllHash -ne $lock.dll_sha256.ToLower()) {
  Remove-Item $dll.FullName -Force
  throw "pdfium.dll distinta: esperado $($lock.dll_sha256), obtenido $dllHash. DLL borrada."
}
Write-Host ("pdfium.dll listo: {0} ({1:N1} MB)" -f $dll.FullName, ($dll.Length / 1MB))
