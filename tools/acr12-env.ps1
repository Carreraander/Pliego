<#
.SYNOPSIS
  Entorno de compilación de Pliego en acr12, SOLO para la terminal actual (no cambia variables globales).

.DESCRIPTION
  Uso (con punto, para que afecte a tu sesión):   . .\tools\acr12-env.ps1
  - LIBCLANG_PATH -> LLVM 19 (mupdf-sys no compila con clang 23).
  - CARGO_TARGET_DIR -> D:\dev-cache\target\<nombre del clon>, para no llenar C: ni bloquear el target con otros flujos
    (en acr12 hay un CARGO_TARGET_DIR global que usan otros proyectos; aquí solo lo anulamos en esta terminal).
#>
# Todo va dentro de & { } para no dejar variables sueltas ($d, $repo, $cache...) en la terminal de quien
# lo carga con punto: solo deben quedar las variables de entorno ($env:...), que no dependen del ámbito.
& {
    param($raiz)
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
    $repo = Split-Path -Parent $raiz
    # El disco C: tiene poco espacio: los target van a D:, uno por clon para que los flujos no se bloqueen entre sí.
    $targetRoot = if (Test-Path 'D:\') { 'D:\dev-cache\target' } else { Join-Path $repo 'target' }
    $env:CARGO_TARGET_DIR = if ($targetRoot -like 'D:*') { Join-Path $targetRoot (Split-Path -Leaf $repo) } else { $targetRoot }
    # C: tiene poco espacio: temporales y cachés de herramientas a D: (solo en esta terminal).
    if (Test-Path 'D:\') {
        $cache = 'D:\dev-cache'
        foreach ($d in 'tmp', 'npm', 'ms-playwright', 'vcpkg-binary') { New-Item -ItemType Directory -Force -Path (Join-Path $cache $d) | Out-Null }
        $env:TEMP = $env:TMP = Join-Path $cache 'tmp'
        $env:npm_config_cache = Join-Path $cache 'npm'
        $env:PLAYWRIGHT_BROWSERS_PATH = Join-Path $cache 'ms-playwright'
        $env:VCPKG_DEFAULT_BINARY_CACHE = Join-Path $cache 'vcpkg-binary'
        # CARGO_HOME: se respeta el que ya haya (en acr12 ya está en D:); si no hay ninguno, uno en D:.
        if (-not $env:CARGO_HOME) { $env:CARGO_HOME = Join-Path $cache 'cargo' }
    }
    # acr12 es compartido por muchos flujos: cada compilación usa como mucho 4 núcleos.
    # Las compilaciones y pruebas largas se lanzan con tools\acr12-turno.ps1 (un solo turno, por orden de llegada).
    $env:CARGO_BUILD_JOBS = '4'
    Write-Host "LIBCLANG_PATH=$env:LIBCLANG_PATH  CARGO_TARGET_DIR=$env:CARGO_TARGET_DIR"
    Write-Host "TEMP=$env:TEMP  npm=$env:npm_config_cache  CARGO_HOME=$env:CARGO_HOME"
} $PSScriptRoot
