<#
.SYNOPSIS
  Entorno de compilación de Pliego en acr12, SOLO para la terminal actual (no cambia variables globales).

.DESCRIPTION
  Uso (con punto, para que afecte a tu sesión):   . .\tools\acr12-env.ps1
  - LIBCLANG_PATH -> LLVM 19 (mupdf-sys no compila con clang 23).
  - CARGO_TARGET_DIR -> D:\dev-cache\target\<nombre del clon>, para no llenar C: ni bloquear el target con otros flujos
    (en acr12 hay un CARGO_TARGET_DIR global que usan otros proyectos; aquí solo lo anulamos en esta terminal).
#>
$llvm19 = 'D:\LLVM19\bin'
if (-not (Test-Path "$llvm19\libclang.dll")) {
    Write-Warning "Falta LLVM 19 en D:\LLVM19; ejecuta tools\setup-acr12.ps1."
} else {
    $env:LIBCLANG_PATH = $llvm19
    # bindgen (mupdf-sys) necesita clang.exe en el PATH; sin él falta max_align_t aunque LIBCLANG_PATH sea correcto.
    $env:Path = "$llvm19;$env:Path"
}

# Tesseract 5 y Leptonica (DLL de vcpkg, triplete x64-windows-release) para pliego-ocr: ver docs/ocr.md.
$vcpkgRoot = if ($env:VCPKG_ROOT) { $env:VCPKG_ROOT } else { Join-Path $env:USERPROFILE 'vcpkg' }
$vi = Join-Path $vcpkgRoot 'installed\x64-windows-release'
$tess = Get-ChildItem "$vi\lib" -Filter 'tesseract*.lib' -ErrorAction SilentlyContinue | Select-Object -First 1
$lept = Get-ChildItem "$vi\lib" -Filter 'leptonica*.lib' -ErrorAction SilentlyContinue | Select-Object -First 1
if ($tess -and $lept) {
    $env:TESSERACT_LIB_DIR = "$vi\lib"
    $env:TESSERACT_LIB_NAME = $tess.BaseName
    $env:LEPTONICA_LIB_NAME = $lept.BaseName
    $env:Path = "$vi\bin;$env:Path"
} else {
    Write-Warning "Tesseract no encontrado en $vi; ejecuta tools\ocr\setup-tesseract-windows.ps1 (ver docs/ocr.md)."
}
$repo = Split-Path -Parent $PSScriptRoot
# El disco C: tiene poco espacio: los target van a D:, uno por clon para que los flujos no se bloqueen entre sí.
$targetRoot = if (Test-Path 'D:\') { 'D:\dev-cache\target' } else { Join-Path $repo 'target' }
$env:CARGO_TARGET_DIR = if ($targetRoot -like 'D:*') { Join-Path $targetRoot (Split-Path -Leaf $repo) } else { $targetRoot }
Write-Host "LIBCLANG_PATH=$env:LIBCLANG_PATH  CARGO_TARGET_DIR=$env:CARGO_TARGET_DIR"
