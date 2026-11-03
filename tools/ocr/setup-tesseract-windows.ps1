<#
.SYNOPSIS
  Instala Tesseract + Leptonica con vcpkg y deja listas las variables que lee `tesseract-sys`.

.DESCRIPTION
  Reproducible: usa el vcpkg del equipo (VCPKG_ROOT o C:\vcpkg) con el triplete x64-windows-static-md
  (biblioteca estática, runtime dinámico de MSVC), de modo que no hay DLL de Tesseract que empaquetar.
  Con -GithubEnv escribe las variables en el archivo de entorno de GitHub Actions; si no, las deja en la sesión.
  Con -TessdataDir descarga eng, spa y osd (tessdata_fast) en esa carpeta.

.EXAMPLE
  ./tools/ocr/setup-tesseract-windows.ps1 -TessdataDir "$env:LOCALAPPDATA\Pliego\tessdata"
#>
param(
    [string]$GithubEnv = "",
    [string]$Triplet = "x64-windows-static-md",
    [string]$TessdataDir = ""
)
$ErrorActionPreference = "Stop"

$vcpkgRoot = if ($env:VCPKG_ROOT) { $env:VCPKG_ROOT } elseif (Test-Path "C:\vcpkg") { "C:\vcpkg" } else { "" }
if (-not $vcpkgRoot) {
    $vcpkgRoot = Join-Path $env:USERPROFILE "vcpkg"
    git clone --depth 1 https://github.com/microsoft/vcpkg $vcpkgRoot
    & "$vcpkgRoot\bootstrap-vcpkg.bat" -disableMetrics
}
& "$vcpkgRoot\vcpkg.exe" install "tesseract:$Triplet" "leptonica:$Triplet"
if ($LASTEXITCODE -ne 0) { throw "vcpkg falló al instalar Tesseract" }

$installed = Join-Path $vcpkgRoot "installed\$Triplet"

# Devuelve nombres de las bibliotecas de una carpeta: Tesseract, Leptonica y el resto (dependencias estáticas).
# Se excluyen jpeg/turbojpeg: MuPDF trae su propio libjpeg y enlazar el de vcpkg duplicaría símbolos
# (no usamos JPEG de Leptonica: las imágenes se pasan ya descomprimidas).
function Get-LibSet([string]$dir) {
    $all = Get-ChildItem $dir -Filter "*.lib"
    $tess = $all | Where-Object { $_.Name -like "tesseract*" } | Select-Object -First 1
    $lept = $all | Where-Object { $_.Name -like "leptonica*" } | Select-Object -First 1
    if (-not $tess -or -not $lept) { throw "No se encontraron las bibliotecas de Tesseract en $dir" }
    $extra = ($all | Where-Object { $_.Name -notin @($tess.Name, $lept.Name) -and $_.BaseName -notmatch "^(turbo)?jpeg" } |
        ForEach-Object { $_.BaseName }) -join ","
    # libcurl y las demás estáticas necesitan estas bibliotecas del sistema.
    $extra += ",crypt32,ws2_32,wldap32,normaliz,bcrypt,advapi32,secur32,user32,xmllite,iphlpapi,ole32,shlwapi"
    return @{ Dir = $dir; Tess = $tess.BaseName; Lept = $lept.BaseName; Extra = $extra }
}
# Las bibliotecas de release llevan el runtime MD y las de debug MDd; Cargo en perfil dev enlaza con MDd (igual que
# MuPDF), así que `tesseract-sys` usa las variables *_DEBUG cuando PROFILE=debug.
$rel = Get-LibSet (Join-Path $installed "lib")
$dbg = Get-LibSet (Join-Path $installed "debug\lib")
$vars = @{
    TESSERACT_LIB_DIR          = $rel.Dir
    TESSERACT_LIB_NAME         = $rel.Tess
    LEPTONICA_LIB_NAME         = $rel.Lept
    TESSERACT_EXTRA_LIBS       = $rel.Extra
    TESSERACT_LIB_DIR_DEBUG    = $dbg.Dir
    TESSERACT_LIB_NAME_DEBUG   = $dbg.Tess
    LEPTONICA_LIB_NAME_DEBUG   = $dbg.Lept
    TESSERACT_EXTRA_LIBS_DEBUG = $dbg.Extra
}
foreach ($k in $vars.Keys) {
    if ($GithubEnv) { "$k=$($vars[$k])" | Out-File -FilePath $GithubEnv -Append -Encoding utf8 }
    else { Set-Item -Path "Env:$k" -Value $vars[$k] }
}

if ($TessdataDir) {
    New-Item -ItemType Directory -Force $TessdataDir | Out-Null
    foreach ($l in "eng", "spa", "osd") {
        $dest = Join-Path $TessdataDir "$l.traineddata"
        if (-not (Test-Path $dest)) {
            Invoke-WebRequest "https://github.com/tesseract-ocr/tessdata_fast/raw/main/$l.traineddata" -OutFile $dest
        }
    }
}
Write-Host "Tesseract listo en $installed"
