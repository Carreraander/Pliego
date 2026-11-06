<#
.SYNOPSIS
  Banco de PDF grandes (hueco P1 #11): un escaneado de ~1 GB y un texto de 5 000 páginas, con el worker real.
.DESCRIPTION
  Mide, con `big-bench`, el tiempo y la memoria privada de app y worker al abrir, desplazarse, buscar y pedir texto, y
  luego (si no se pasa -SinEdicion) comenta y deshace sobre una copia del escaneado de 1 GB para ver la memoria de la
  app durante la edición y el historial de deshacer.

  Criterios (docs/qa/pdf-grandes.md):
    - worker en reposo con el PDF de 1 GB: no crece con el tamaño del archivo (esperado < 100 MB; pico al abrir
      transitorio ~ tamaño del archivo, siempre por debajo del límite de 2 GB del job object).
    - app + worker: <= 150 MB en reposo con cualquier PDF abierto (sin contar la edición, que reescribe el PDF entero).
    - tras editar y deshacer la app no retiene copias del PDF (< 300 MB en reposo con el de 1 GB).

  Uso (en acr12, todo en D: y con el turno):
      . .\tools\acr12-env.ps1
      .\tools\acr12-turno.ps1 -Flujo w44 -- .\tools\bench-pdf-grandes.ps1
      .\tools\bench-pdf-grandes.ps1 -SinEdicion           # solo lectura (más rápido)
  Los PDF se generan en -Dir (por defecto D:\dev-cache\big; hacen falta ~3 GB libres) y se reutilizan.
  Resultados: docs\qa\pdf-grandes\<fecha>-<equipo>-<caso>.json.
#>
param(
    [string]$Dir = 'D:\dev-cache\big',
    [int]$ScanPages = 300,
    [double]$ImageMb = 3.5,
    [int]$TextPages = 5000,
    [string]$Pdfium = 'D:\dev-cache\pdfium',
    [switch]$SinEdicion
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
Set-Location $repo
$target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $repo 'target' }

# Compilar el banco y el worker (release: es lo que se empaqueta).
cargo build --release -p pliego-qa --bin big-bench -p pliego-worker
if ($LASTEXITCODE -ne 0) { throw 'cargo build falló' }
$bench = Join-Path $target 'release\big-bench.exe'
$env:PLIEGO_WORKER = Join-Path $target 'release\pliego-worker.exe'
$env:PLIEGO_ENGINE = 'pdfium'

# PDFium: la versión fijada en tools/pdfium/pdfium.lock.json, por ruta absoluta.
$dll = Join-Path $Pdfium 'pdfium.dll'
if (-not (Test-Path $dll)) { & (Join-Path $repo 'tools\pdfium\fetch-pdfium.ps1') -OutDir $Pdfium }
$env:PLIEGO_PDFIUM_DLL = (Resolve-Path $dll).Path

New-Item -ItemType Directory -Force -Path $Dir | Out-Null
$scan = Join-Path $Dir 'escaneado-1gb.pdf'
$text = Join-Path $Dir 'texto-5000.pdf'
if (-not (Test-Path $scan)) { & $bench gen-scan $scan --pages $ScanPages --image-mb $ImageMb }
if (-not (Test-Path $text)) { & $bench gen-text $text --pages $TextPages }

$fecha = Get-Date -Format 'yyyy-MM-dd-HHmm'
$salida = Join-Path $repo 'docs\qa\pdf-grandes'
New-Item -ItemType Directory -Force -Path $salida | Out-Null
foreach ($caso in @(
        @{ n = 'escaneado-1gb'; f = $scan; busca = $ScanPages },
        @{ n = 'texto-5000'; f = $text; busca = $TextPages })) {
    Write-Host "`n=== $($caso.n) ==="
    & $bench run $caso.f --label $caso.n --search-pages $caso.busca --json (Join-Path $salida "$fecha-$env:COMPUTERNAME-$($caso.n).json")
    if ($LASTEXITCODE -ne 0) { throw "big-bench falló en $($caso.n)" }
}

if (-not $SinEdicion) {
    Write-Host "`n=== comentar y deshacer sobre el escaneado de 1 GB (copia en $Dir) ==="
    $env:PLIEGO_BIG_PDF = $scan
    cargo test --release -p pliego-app-service --test pdf_grande -- --ignored --nocapture
    if ($LASTEXITCODE -ne 0) { throw 'la prueba de edición de un PDF grande falló' }
}
Write-Host "`nListo. Resultados en $salida"
