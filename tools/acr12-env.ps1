<#
.SYNOPSIS
  Entorno de compilación de Pliego en acr12, SOLO para la terminal actual (no cambia variables globales).

.DESCRIPTION
  Uso (con punto, para que afecte a tu sesión):   . .\tools\acr12-env.ps1
  Envoltorio de tools\pliego-env.ps1, que detecta D: y deja en acr12 lo mismo que antes:
  - LIBCLANG_PATH y PATH -> D:\LLVM19\bin (bindgen de tesseract-sys necesita clang.exe en el PATH).
  - CARGO_TARGET_DIR -> D:\dev-cache\target\<nombre del clon> (en acr12 hay un CARGO_TARGET_DIR global que usan
    otros proyectos; aquí solo lo anulamos en esta terminal).
  - TEMP/TMP, npm, Playwright y vcpkg en D:\dev-cache; Tesseract de vcpkg; CARGO_BUILD_JOBS=4.
#>
. (Join-Path $PSScriptRoot 'pliego-env.ps1')
