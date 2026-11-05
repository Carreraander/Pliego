<#
.SYNOPSIS
  Instala qpdf y veraPDF por línea de órdenes en D:\dev-cache\validadores para el arnés de comparación (W34).

.DESCRIPTION
  - qpdf: último zip msvc64 de https://github.com/qpdf/qpdf/releases (se resuelve por la API de GitHub). Se descomprime en
    <Destino>\qpdf y su bin\ queda disponible en <Destino>\bin.
  - veraPDF: instalador "greenfield" de https://software.verapdf.org/ (necesita Java 11 o superior en el PATH), instalado en
    modo desatendido en <Destino>\verapdf.
  Imprime el SHA-256 de cada descarga (para anotarlo en el informe del turno). No toca el PATH global ni el registro:
  añade <Destino>\bin a esta terminal con -Activar o con `. .\tools\qa\instalar-validadores.ps1 -SoloEntorno`.
  Sin cambios en el repositorio. Corre dentro de un turno de acr12:
    . .\tools\acr12-env.ps1
    .\tools\acr12-turno.ps1 -Flujo w34 -- powershell -File .\tools\qa\instalar-validadores.ps1

.EXAMPLE
  .\tools\qa\instalar-validadores.ps1
  . .\tools\qa\instalar-validadores.ps1 -SoloEntorno
  target\release\engine-compare.exe tests\corpus --validate-dir salida\
#>
[CmdletBinding()]
param(
  [string]$Destino = 'D:\dev-cache\validadores',
  [string]$VeraPdfUrl = 'https://software.verapdf.org/rel/1.28/verapdf-greenfield-1.28.2-installer.zip',
  [switch]$SoloEntorno
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$bin = Join-Path $Destino 'bin'
if ($SoloEntorno) {
  $env:PATH = "$bin;$env:PATH"
  Write-Host "PATH de esta terminal ampliado con $bin"
  return
}
New-Item -ItemType Directory -Force $Destino, $bin | Out-Null
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

# --- qpdf ---
$rel = Invoke-RestMethod -Uri 'https://api.github.com/repos/qpdf/qpdf/releases/latest' -Headers @{ 'User-Agent' = 'pliego-qa' }
$asset = $rel.assets | Where-Object { $_.name -match 'msvc64\.zip$' } | Select-Object -First 1
if (-not $asset) { throw "No hay zip msvc64 en la versión $($rel.tag_name) de qpdf." }
$zip = Join-Path $Destino $asset.name
Write-Host "Descargando qpdf $($rel.tag_name): $($asset.browser_download_url)"
Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $zip -UseBasicParsing
Write-Host ("SHA-256 {0}: {1}" -f $asset.name, (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower())
$qdir = Join-Path $Destino 'qpdf'
if (Test-Path $qdir) { Remove-Item $qdir -Recurse -Force }
Expand-Archive $zip -DestinationPath $qdir -Force
$qbin = Get-ChildItem $qdir -Recurse -Filter qpdf.exe | Select-Object -First 1
if (-not $qbin) { throw 'qpdf.exe no aparece tras descomprimir.' }
# Las DLL van junto a qpdf.exe: se copia toda la carpeta bin\ a <Destino>\bin.
Copy-Item (Join-Path $qbin.DirectoryName '*') $bin -Recurse -Force
& (Join-Path $bin 'qpdf.exe') --version | Select-Object -First 1

# --- veraPDF ---
if (-not (Get-Command java -ErrorAction SilentlyContinue)) {
  Write-Warning 'Falta Java (11 o superior) en el PATH: veraPDF no se instala. Instala un JRE (p. ej. Temurin) y repite.'
} else {
  $vzip = Join-Path $Destino 'verapdf-installer.zip'
  Write-Host "Descargando veraPDF: $VeraPdfUrl"
  Invoke-WebRequest -Uri $VeraPdfUrl -OutFile $vzip -UseBasicParsing
  Write-Host ("SHA-256 veraPDF: {0}" -f (Get-FileHash $vzip -Algorithm SHA256).Hash.ToLower())
  $vstage = Join-Path $Destino 'verapdf-instalador'
  if (Test-Path $vstage) { Remove-Item $vstage -Recurse -Force }
  Expand-Archive $vzip -DestinationPath $vstage -Force
  $jar = Get-ChildItem $vstage -Recurse -Filter 'verapdf-izpack-installer-*.jar' | Select-Object -First 1
  if (-not $jar) { throw 'No se encuentra verapdf-izpack-installer-*.jar en el zip.' }
  $vdir = Join-Path $Destino 'verapdf'
  $xml = Join-Path $Destino 'verapdf-auto-install.xml'
  @"
<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<AutomatedInstallation langpack="eng">
  <com.izforge.izpack.panels.htmlhello.HTMLHelloPanel id="welcome"/>
  <com.izforge.izpack.panels.target.TargetPanel id="install_dir"><installpath>$vdir</installpath></com.izforge.izpack.panels.target.TargetPanel>
  <com.izforge.izpack.panels.packs.PacksPanel id="sdk_pack_select">
    <pack index="0" name="veraPDF Runtime" selected="true"/>
    <pack index="1" name="veraPDF Mac and *nix Scripts" selected="false"/>
    <pack index="2" name="veraPDF Validation model" selected="false"/>
    <pack index="3" name="veraPDF Documentation" selected="false"/>
    <pack index="4" name="veraPDF Sample Plugins" selected="false"/>
  </com.izforge.izpack.panels.packs.PacksPanel>
  <com.izforge.izpack.panels.install.InstallPanel id="install"/>
  <com.izforge.izpack.panels.finish.FinishPanel id="finish"/>
</AutomatedInstallation>
"@ | Set-Content -Encoding UTF8 $xml
  & java -jar $jar.FullName $xml
  if ($LASTEXITCODE -ne 0) { throw "El instalador de veraPDF terminó con código $LASTEXITCODE." }
  Copy-Item (Join-Path $vdir 'verapdf.bat') $bin -Force
  Write-Host "veraPDF en $vdir (lanzador: $bin\verapdf.bat; engine-compare busca «verapdf», así que ejecuta con -SoloEntorno)."
}
Write-Host "Listo. Antes de usar el arnés: . .\tools\qa\instalar-validadores.ps1 -SoloEntorno"
