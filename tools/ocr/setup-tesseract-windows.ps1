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
$libDir = Join-Path $installed "lib"
# Las bibliotecas llevan versión en el nombre (tesseract55.lib, leptonica-1.85.0.lib): se buscan.
$tess = Get-ChildItem $libDir -Filter "tesseract*.lib" | Select-Object -First 1
$lept = Get-ChildItem $libDir -Filter "leptonica*.lib" | Select-Object -First 1
if (-not $tess -or -not $lept) { throw "No se encontraron las bibliotecas de Tesseract en $libDir" }

# Dependencias transitivas de una instalación estática (zlib, png, jpeg, tiff, webp, ...): se enlazan todas las .lib.
$extra = (Get-ChildItem $libDir -Filter "*.lib" | Where-Object { $_.Name -notin @($tess.Name, $lept.Name) } |
    ForEach-Object { $_.BaseName }) -join ","
# libcurl (dependencia de Tesseract en vcpkg) y las demás estáticas necesitan estas bibliotecas del sistema.
$extra += ",crypt32,ws2_32,wldap32,normaliz,bcrypt,advapi32,secur32,user32"
$vars = @{
    TESSERACT_LIB_DIR    = $libDir
    TESSERACT_LIB_NAME   = $tess.BaseName
    LEPTONICA_LIB_NAME   = $lept.BaseName
    TESSERACT_EXTRA_LIBS = $extra
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
Write-Host "Tesseract listo en $libDir"
