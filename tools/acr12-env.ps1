<#
.SYNOPSIS
  Entorno de compilación de Pliego en acr12, SOLO para la terminal actual (no cambia variables globales).

.DESCRIPTION
  Uso (con punto, para que afecte a tu sesión):   . .\tools\acr12-env.ps1
  - LIBCLANG_PATH -> LLVM 19 (mupdf-sys no compila con clang 23).
  - CARGO_TARGET_DIR -> .\target de este clon, para no compartir ni bloquear el target con otros flujos
    (en acr12 hay un CARGO_TARGET_DIR global que usan otros proyectos; aquí solo lo anulamos en esta terminal).
#>
$llvm19 = 'C:\LLVM19\bin'
if (-not (Test-Path "$llvm19\libclang.dll")) {
    Write-Warning "Falta LLVM 19 en C:\LLVM19; ejecuta tools\setup-acr12.ps1."
} else {
    $env:LIBCLANG_PATH = $llvm19
}
$repo = Split-Path -Parent $PSScriptRoot
$env:CARGO_TARGET_DIR = Join-Path $repo 'target'
Write-Host "LIBCLANG_PATH=$env:LIBCLANG_PATH  CARGO_TARGET_DIR=$env:CARGO_TARGET_DIR"
