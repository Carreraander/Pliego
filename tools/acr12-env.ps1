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
}
$repo = Split-Path -Parent $PSScriptRoot
# El disco C: tiene poco espacio: los target van a D:, uno por clon para que los flujos no se bloqueen entre sí.
$targetRoot = if (Test-Path 'D:\') { 'D:\dev-cache\target' } else { Join-Path $repo 'target' }
$env:CARGO_TARGET_DIR = if ($targetRoot -like 'D:*') { Join-Path $targetRoot (Split-Path -Leaf $repo) } else { $targetRoot }
Write-Host "LIBCLANG_PATH=$env:LIBCLANG_PATH  CARGO_TARGET_DIR=$env:CARGO_TARGET_DIR"
