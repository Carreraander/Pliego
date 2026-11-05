<#
.SYNOPSIS
  Compila Pliego en release y genera el paquete .paq y el instalador Pliego-Setup-<versión>-x86_64.exe.

.DESCRIPTION
  1. Compila lanzadores, CLI, worker y pliego-setup (cargo, release).
  2. Prepara la carpeta app-<versión> con la app, la CLI, el worker (con pdfium.dll y sus avisos de licencia), los lanzadores y el desinstalador.
  3. Firma (opcional) con Authenticode si SIGN_TOOL y SIGN_ARGS están definidos; vacío = sin firmar.
  4. Empaqueta en dist\pliego-<versión>-x86_64.paq (lo que usa el actualizador, W22).
  5. Pega el .paq al ejecutable del instalador: dist\Pliego-Setup-<versión>-x86_64.exe.

.PARAMETER AppDir
  Carpeta con la app ya compilada (pliego-app.exe y recursos). Si falta y existe app\src-tauri, se compila con Tauri.
.PARAMETER FakeApp
  Usa un pliego-app.exe de pruebas (examples\fake_app.rs): para probar el instalador sin la app real.
.PARAMETER SkipCargo
  No ejecuta cargo (reutiliza target\release).

.EXAMPLE
  .\installer\setup\build.ps1 -FakeApp
  $env:SIGN_TOOL = 'signtool.exe'; $env:SIGN_ARGS = 'sign /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 /a'; .\installer\setup\build.ps1 -AppDir C:\build\app
#>
[CmdletBinding()]
param(
  [string]$Version,
  [string]$OutDir,
  [string]$AppDir,
  [switch]$FakeApp,
  [switch]$SkipCargo
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# Windows PowerShell 5.1 lanzado desde pwsh 7 hereda un PSModulePath con módulos de 7 que no carga (Get-FileHash
# "not recognized"): se reconstruye con los valores de Windows.
if ($PSVersionTable.PSVersion.Major -lt 6) {
  $mp = @([Environment]::GetEnvironmentVariable('PSModulePath', 'User'), [Environment]::GetEnvironmentVariable('PSModulePath', 'Machine')) | Where-Object { $_ }
  if (-not $mp) { $mp = @((Join-Path $PSHOME 'Modules')) }
  $env:PSModulePath = $mp -join ';'
}

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if (-not $OutDir) { $OutDir = Join-Path $repo 'dist' }
if (-not $Version) {
  $m = Select-String -Path (Join-Path $repo 'Cargo.toml') -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1
  $Version = $m.Matches[0].Groups[1].Value
}
$target = if ($env:CARGO_TARGET_DIR) { Join-Path $env:CARGO_TARGET_DIR 'release' } else { Join-Path $repo 'target\release' }
Write-Host "Pliego $Version -> $OutDir"

function Invoke-Checked([string]$exe, [string[]]$argv) {
  # Canalizar la salida obliga a PowerShell a esperar también a los ejecutables de subsistema GUI (pliego-setup.exe).
  & $exe @argv | Out-Host
  if ($LASTEXITCODE -ne 0) { throw "$exe terminó con código $LASTEXITCODE" }
}

function Sign-File([string]$path) {
  if (-not $env:SIGN_TOOL) { return }
  Write-Host "Firmando $(Split-Path $path -Leaf)"
  $argv = @()
  if ($env:SIGN_ARGS) { $argv += ($env:SIGN_ARGS -split '\s+') }
  Invoke-Checked $env:SIGN_TOOL ($argv + $path)
}

# 1. Compilación
if (-not $SkipCargo) {
  Push-Location $repo
  try {
    $pkgs = @('-p', 'pliego-launcher', '-p', 'pliego-setup')
    # Con -FakeApp no se compilan la CLI ni el worker (necesitan PDFium y Tesseract): basta para probar el instalador.
    if (-not $FakeApp) { $pkgs += @('-p', 'pliego-cli', '-p', 'pliego-worker') }
    Invoke-Checked 'cargo' (@('build', '--release', '--locked') + $pkgs)
    if ($FakeApp) { Invoke-Checked 'cargo' @('build', '--release', '--locked', '-p', 'pliego-setup', '--example', 'fake_app') }
  } finally { Pop-Location }
}

# 2. Carpeta de la versión
$stage = Join-Path $OutDir "stage\app-$Version"
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Path $stage, (Join-Path $stage 'launcher') | Out-Null

if ($AppDir) {
  Copy-Item (Join-Path $AppDir '*') $stage -Recurse -Force
} elseif ($FakeApp) {
  Copy-Item (Join-Path $target 'examples\fake_app.exe') (Join-Path $stage 'pliego-app.exe')
} elseif (Test-Path (Join-Path $repo 'app\src-tauri')) {
  Push-Location (Join-Path $repo 'app')
  try {
    Invoke-Checked 'npm' @('ci')
    Invoke-Checked 'npm' @('run', 'tauri', 'build', '--', '--no-bundle')
  } finally { Pop-Location }
  $built = Join-Path $repo 'app\src-tauri\target\release\pliego-app.exe'
  if (-not (Test-Path $built)) { $built = Join-Path $target 'pliego-app.exe' }
  Copy-Item $built (Join-Path $stage 'pliego-app.exe')
} else {
  throw 'No hay app que empaquetar: usa -AppDir <carpeta>, -FakeApp o compila la app (app\src-tauri).'
}
if (-not (Test-Path (Join-Path $stage 'pliego-app.exe'))) { throw 'Falta pliego-app.exe en la carpeta de la app.' }

foreach ($n in 'pliego.exe', 'pliego-worker.exe') {
  $src = Join-Path $target $n
  if (Test-Path $src) { Copy-Item $src (Join-Path $stage $n) }
  elseif (-not $FakeApp) { throw "Falta $n en $target" }
}
# pdfium.dll (W37, W41): render, texto y búsqueda. Lo cargan el worker, la CLI y la app, por ruta absoluta desde la
# carpeta de su ejecutable (nunca por PATH); por eso va en la raíz de la carpeta de la versión, junto a todos ellos. Versión y hashes fijados en tools\pdfium\pdfium.lock.json: fetch-pdfium.ps1 verifica el .tgz y la DLL.
# PDFIUM_DIR (opcional) apunta a una carpeta ya descargada con pdfium.dll, LICENSE.pdfium y licenses\ (caché de acr12).
# Los avisos de licencia (BSD-3/Apache-2.0 y terceros) se instalan en licenses\pdfium\ junto a la DLL.
if (-not $FakeApp) {
  $pdfiumDir = if ($env:PDFIUM_DIR) { $env:PDFIUM_DIR } else { Join-Path $OutDir 'pdfium' }
  $pdfiumLock = Get-Content (Join-Path $repo 'tools\pdfium\pdfium.lock.json') -Raw | ConvertFrom-Json
  $pdfiumDll = Join-Path $pdfiumDir 'pdfium.dll'
  $cached = (Test-Path $pdfiumDll) -and ((Get-FileHash $pdfiumDll -Algorithm SHA256).Hash.ToLower() -eq $pdfiumLock.dll_sha256.ToLower())
  if (-not $cached) { & (Join-Path $repo 'tools\pdfium\fetch-pdfium.ps1') -OutDir $pdfiumDir | Out-Host }
  if (-not (Test-Path $pdfiumDll)) { throw "Falta pdfium.dll en $pdfiumDir: el worker no podría dibujar con PDFium." }
  Copy-Item $pdfiumDll (Join-Path $stage 'pdfium.dll')
  $pdfiumLic = Join-Path $stage 'licenses\pdfium'
  New-Item -ItemType Directory -Force -Path $pdfiumLic | Out-Null
  Copy-Item (Join-Path $pdfiumDir 'LICENSE.pdfium') (Join-Path $pdfiumLic 'LICENSE.pdfium')
  if (Test-Path (Join-Path $pdfiumDir 'licenses')) { Copy-Item (Join-Path $pdfiumDir 'licenses\*') $pdfiumLic -Recurse -Force }
  Write-Host "pdfium.dll $($pdfiumLock.version) ($($pdfiumLock.dll_version)) incluida junto a la app, la CLI y el worker"
}
# DLL de Tesseract y Leptonica (vcpkg, DLL dinámicas) junto a pliego.exe: sin ellas la CLI sale con 0xC0000135 en un equipo
# sin vcpkg. Se toma la carpeta bin de vcpkg a partir de TESSERACT_LIB_DIR (..\bin) o, si no, del PATH.
if (-not $FakeApp) {
  $dllDirs = @()
  if ($env:TESSERACT_LIB_DIR) { $dllDirs += (Join-Path (Split-Path $env:TESSERACT_LIB_DIR -Parent) 'bin') }
  $dllDirs += ($env:PATH -split ';' | Where-Object { $_ })
  $tessDll = $dllDirs | Where-Object { $_ -and (Test-Path (Join-Path $_ 'tesseract*.dll')) } | Select-Object -First 1
  if (-not $tessDll) { throw 'No se encuentran las DLL de Tesseract (define TESSERACT_LIB_DIR o carga tools\ocr\setup-tesseract-windows.ps1): la CLI no arrancaría.' }
  $dlls = Get-ChildItem $tessDll -Filter *.dll
  if (-not ($dlls | Where-Object Name -like 'leptonica*')) { throw "Faltan las DLL de Leptonica en $tessDll." }
  $dlls | Copy-Item -Destination $stage
  Write-Host "DLL de Tesseract/Leptonica: $($dlls.Count) desde $tessDll"
  # Licencias de las bibliotecas empaquetadas (vcpkg: share\<paquete>\copyright).
  $share = Join-Path (Split-Path $tessDll -Parent) 'share'
  if (Test-Path $share) {
    $lic = Join-Path $stage 'licenses'
    New-Item -ItemType Directory -Force -Path $lic | Out-Null
    Get-ChildItem $share -Directory | ForEach-Object {
      $c = Join-Path $_.FullName 'copyright'
      if (Test-Path $c) { Copy-Item $c (Join-Path $lic "$($_.Name).txt") }
    }
  }
}
Copy-Item (Join-Path $target 'pliego-setup.exe')         (Join-Path $stage 'pliego-setup.exe')
Copy-Item (Join-Path $target 'pliego-launcher.exe')      (Join-Path $stage 'launcher\Pliego.exe')
Copy-Item (Join-Path $target 'pliego-launcher-cli.exe')  (Join-Path $stage 'launcher\pliego-cli.exe')  # NTFS no distingue Pliego.exe/pliego.exe
Copy-Item (Join-Path $repo 'LICENSE') (Join-Path $stage 'LICENSE') -ErrorAction SilentlyContinue
# Avisos de terceros y texto de la licencia de cambio (AGPL-3.0-or-later), junto a los de las DLL.
$legal = Join-Path $stage 'licenses'
New-Item -ItemType Directory -Force -Path $legal | Out-Null
Copy-Item (Join-Path $repo 'THIRD-PARTY.md') (Join-Path $legal 'THIRD-PARTY.md') -ErrorAction SilentlyContinue
Copy-Item (Join-Path $repo 'docs\legal\dependencias-rust.md') (Join-Path $legal 'dependencias-rust.md') -ErrorAction SilentlyContinue
Copy-Item (Join-Path $repo 'LICENSES\AGPL-3.0-or-later.txt') (Join-Path $legal 'AGPL-3.0-or-later.txt') -ErrorAction SilentlyContinue

# 3. Firma (opcional): todo lo ejecutable antes de empaquetar
Get-ChildItem $stage -Recurse -Include *.exe, *.dll | ForEach-Object { Sign-File $_.FullName }

# 4. Paquete para el actualizador
$paq = Join-Path $OutDir "pliego-$Version-x86_64.paq"
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$setupBase = Join-Path $stage 'pliego-setup.exe'   # ya firmado: se pega el .paq DESPUÉS de la firma
Invoke-Checked $setupBase @('--make-paq', '--dir', $stage, '--version', $Version, '--out', $paq)

# 5. Instalador = ejecutable base firmado + .paq + pie. Los datos añadidos tras el certificado no invalidan la firma.
$setup = Join-Path $OutDir "Pliego-Setup-$Version-x86_64.exe"
Invoke-Checked $setupBase @('--pack', '--base', $setupBase, '--paq', $paq, '--out', $setup)

# Sumas de comprobación
$lines = foreach ($f in @($paq, $setup)) { '{0}  {1}' -f (Get-FileHash $f -Algorithm SHA256).Hash.ToLower(), (Split-Path $f -Leaf) }
$lines | Set-Content (Join-Path $OutDir "SHA256SUMS-$Version.txt") -Encoding ascii

$mb = [math]::Round((Get-Item $setup).Length / 1MB, 1)
Write-Host "Listo: $setup ($mb MB)"
if ($mb -ge 40) { Write-Warning "El instalador supera los 40 MB del objetivo." }
