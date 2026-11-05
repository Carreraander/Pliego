<#
.SYNOPSIS
  Entorno de compilación de Pliego en un equipo Windows de pruebas, SOLO para la terminal actual
  (no cambia variables globales). Lo cargan tools\acr12-env.ps1 (acr12) y tools\pc-env.ps1 (pc).

.DESCRIPTION
  Uso (con punto, para que afecte a tu sesión):   . .\tools\pliego-env.ps1
  Detecta el disco de trabajo: D:\dev-cache si existe D: (acr12) y si no C:\dev-cache (pc, un solo disco).
  Se puede forzar con $env:PLIEGO_CACHE (carpeta de cachés) y $env:PLIEGO_LLVM (carpeta bin de LLVM).
  - LIBCLANG_PATH y PATH -> LLVM 19 (D:\LLVM19, C:\dev-tools\LLVM19) o, si no hay, el LLVM de Program Files.
    bindgen (tesseract-sys) necesita clang.exe en el PATH; sin él falta max_align_t.
  - CARGO_TARGET_DIR -> <cache>\target\<nombre del clon>, uno por clon para que los flujos no se bloqueen.
  - TEMP/TMP, caché de npm, navegadores de Playwright y caché binaria de vcpkg en <cache>.
  - Tesseract/Leptonica de vcpkg (VCPKG_ROOT, ~\vcpkg, <disco>\dev-tools\vcpkg o C:\vcpkg).
  - CARGO_BUILD_JOBS=4.
#>
# Todo va dentro de & { } para no dejar variables sueltas en la terminal de quien lo carga con punto:
# solo deben quedar las variables de entorno ($env:...), que no dependen del ámbito.
& {
    param($raiz)
    $disco = if (Test-Path 'D:\') { 'D:' } else { 'C:' }
    $cache = if ($env:PLIEGO_CACHE) { $env:PLIEGO_CACHE } else { "$disco\dev-cache" }

    # --- LLVM ------------------------------------------------------------------------------------------
    $candidatos = @($env:PLIEGO_LLVM, 'D:\LLVM19\bin', "$disco\dev-tools\LLVM19\bin", "$env:ProgramFiles\LLVM\bin") | Where-Object { $_ }
    # Sin Join-Path: falla si la unidad (D:) no existe.
    $llvm = $candidatos | Where-Object { Test-Path "$_\libclang.dll" } | Select-Object -First 1
    if (-not $llvm) {
        Write-Warning "No hay LLVM con libclang.dll ($($candidatos -join ', ')); ejecuta tools\setup-windows.ps1."
    } else {
        if ($llvm -notlike '*LLVM19*') { Write-Warning "Uso el LLVM de $llvm (no es LLVM 19; con clang.exe en el PATH suele bastar)." }
        $env:LIBCLANG_PATH = $llvm
        $env:Path = "$llvm;$env:Path"
    }

    # --- Tesseract 5 y Leptonica (vcpkg, x64-windows-release): ver docs/ocr.md --------------------------
    $vcpkgs = @($env:VCPKG_ROOT, (Join-Path $env:USERPROFILE 'vcpkg'), "$disco\dev-tools\vcpkg", 'C:\vcpkg') | Where-Object { $_ }
    $vi = $vcpkgs | ForEach-Object { Join-Path $_ 'installed\x64-windows-release' } |
        Where-Object { Get-ChildItem "$_\lib" -Filter 'tesseract*.lib' -ErrorAction SilentlyContinue } | Select-Object -First 1
    if ($vi) {
        $tess = Get-ChildItem "$vi\lib" -Filter 'tesseract*.lib' | Select-Object -First 1
        $lept = Get-ChildItem "$vi\lib" -Filter 'leptonica*.lib' -ErrorAction SilentlyContinue | Select-Object -First 1
        $env:TESSERACT_LIB_DIR = "$vi\lib"
        $env:TESSERACT_LIB_NAME = $tess.BaseName
        if ($lept) { $env:LEPTONICA_LIB_NAME = $lept.BaseName }
        $env:Path = "$vi\bin;$env:Path"
    } else {
        Write-Warning "Tesseract no encontrado en vcpkg ($($vcpkgs -join ', ')); ejecuta tools\setup-windows.ps1 (ver docs/ocr.md)."
    }

    # --- Herramientas opcionales de prueba de tools\setup-windows.ps1 -ConExtras (veraPDF con su JRE) -------
    $jre = Get-ChildItem "$disco\dev-tools\jre\*\bin\java.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($jre) { $env:JAVA_HOME = Split-Path (Split-Path $jre.FullName); $env:Path = "$(Split-Path $jre.FullName);$env:Path" }
    if (Test-Path "$disco\dev-tools\veraPDF\verapdf.bat") { $env:PLIEGO_VERAPDF = "$disco\dev-tools\veraPDF\verapdf.bat" }

    # --- Visual Studio sin C++ instalado más nuevo que el que tiene C++ (pc: VS 18 Insiders + Build Tools 2022) ---
    # Las dependencias en C (vía cc) usan siempre el MSBuild del VS más nuevo, y clang (bindgen) también lo prefiere: si ese VS no
    # tiene el componente C++, falla («Microsoft.Cpp.Default.props was not found», «'setjmp.h' file not found»).
    # Arreglo solo para esta terminal y nuestros target: entorno vcvars64 del VS con C++ (INCLUDE/LIB para clang,
    # VisualStudioVersion para elegir el PlatformToolset) y un Directory.Build.rsp en <cache>\target que MSBuild lee
    # solo para lo que se compila ahí, con VsInstallRoot y VCTargetsPath de ese VS.
    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    $rsp = Join-Path (Join-Path $cache 'target') 'Directory.Build.rsp'
    if (Test-Path $vswhere) {
        $vsNuevo = & $vswhere -latest -prerelease -products * -property installationPath
        $vsCpp = & $vswhere -latest -prerelease -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        if ($vsCpp -and $vsNuevo -and ($vsNuevo -ne $vsCpp)) {
            $vcTargets = Get-ChildItem "$vsCpp\MSBuild\Microsoft\VC\v*" -Directory -ErrorAction SilentlyContinue | Sort-Object Name | Select-Object -Last 1
            if (-not $env:VCINSTALLDIR -or $env:VCINSTALLDIR -notlike "$vsCpp*") {
                $env:Path = "$(Split-Path $vswhere);$env:Path"  # vcvars64.bat llama a vswhere.exe
                cmd /c "`"$vsCpp\VC\Auxiliary\Build\vcvars64.bat`" >nul && set" | ForEach-Object {
                    if ($_ -match '^([^=]+)=(.*)$') { Set-Item "env:$($Matches[1])" $Matches[2] }
                }
            }
            if ($vcTargets) {
                New-Item -ItemType Directory -Force -Path (Split-Path $rsp) | Out-Null
                $lineas = @("/p:VsInstallRoot=`"$vsCpp`"", "/p:VCTargetsPath=`"$($vcTargets.FullName)\\`"")
                if ((Get-Content $rsp -ErrorAction SilentlyContinue) -join "`n" -ne ($lineas -join "`n")) { Set-Content -Path $rsp -Value $lineas -Encoding ascii }
            }
            Write-Host "Visual Studio: $vsNuevo no tiene C++; uso $vsCpp (vcvars64 y $rsp)."
        } elseif (Test-Path $rsp) {
            Remove-Item $rsp  # ya no hace falta (p. ej. se instaló C++ en el VS nuevo)
        }
    }

    # --- Target, temporales y cachés en el disco de trabajo ----------------------------------------------
    $repo = Split-Path -Parent $raiz
    $env:CARGO_TARGET_DIR = Join-Path (Join-Path $cache 'target') (Split-Path -Leaf $repo)
    if ($env:PLIEGO_TARGET_COMPARTIDO -eq '1') { $env:CARGO_TARGET_DIR = Join-Path (Join-Path $cache 'target') 'compartido' }  # un solo target para todos los clones (equipos con poco disco)
    foreach ($d in 'tmp', 'npm', 'ms-playwright', 'vcpkg-binary', 'turnos') { New-Item -ItemType Directory -Force -Path (Join-Path $cache $d) | Out-Null }
    $env:TEMP = $env:TMP = Join-Path $cache 'tmp'
    $env:npm_config_cache = Join-Path $cache 'npm'
    $env:PLAYWRIGHT_BROWSERS_PATH = Join-Path $cache 'ms-playwright'
    $env:VCPKG_DEFAULT_BINARY_CACHE = Join-Path $cache 'vcpkg-binary'
    # CARGO_HOME: se respeta el que ya haya (en acr12 está en D:). Si no hay y existe D:, uno en D:;
    # en un equipo de un solo disco (pc) se deja el de siempre (~\.cargo).
    if (-not $env:CARGO_HOME -and $disco -eq 'D:') { $env:CARGO_HOME = Join-Path $cache 'cargo' }
    $env:PLIEGO_CACHE = $cache
    # Equipo de un solo disco (pc, C: pequeño): sin incremental ni información de depuración en dev, el target
    # ocupa mucho menos. Se respeta lo que ya haya puesto quien carga el script.
    if ($disco -eq 'C:') {
        if (-not $env:CARGO_INCREMENTAL) { $env:CARGO_INCREMENTAL = '0' }
        if (-not $env:CARGO_PROFILE_DEV_DEBUG) { $env:CARGO_PROFILE_DEV_DEBUG = '0' }
    }

    # Los equipos de pruebas son compartidos: cada compilación usa como mucho 4 núcleos.
    # Lo pesado se lanza con tools\pliego-turno.ps1 (acr12-turno.ps1 / pc-turno.ps1): un solo turno, FIFO.
    $env:CARGO_BUILD_JOBS = '4'
    Write-Host "LIBCLANG_PATH=$env:LIBCLANG_PATH  CARGO_TARGET_DIR=$env:CARGO_TARGET_DIR"
    Write-Host "TEMP=$env:TEMP  npm=$env:npm_config_cache  CARGO_HOME=$env:CARGO_HOME"
} $PSScriptRoot
