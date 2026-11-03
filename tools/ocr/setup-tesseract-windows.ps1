<#
.SYNOPSIS
  Instala Tesseract + Leptonica con vcpkg y deja listas las variables que lee `tesseract-sys`.

.DESCRIPTION
  Reproducible: usa el vcpkg del equipo (VCPKG_ROOT o C:\vcpkg) con el triplete x64-windows
  (DLL dinámicas): evita choques de símbolos y de runtime con el MuPDF estático. Las DLL de `<vcpkg>\installed\x64-windows\bin`
  (tesseract55.dll y dependencias) son las que hay que empaquetar con la aplicación.
  Con -GithubEnv escribe las variables en el archivo de entorno de GitHub Actions; si no, las deja en la sesión.
  Con -TessdataDir descarga eng, spa y osd (tessdata_fast) en esa carpeta.

.EXAMPLE
  ./tools/ocr/setup-tesseract-windows.ps1 -TessdataDir "$env:LOCALAPPDATA\Pliego\tessdata"
#>
param(
    [string]$GithubEnv = "",
    [string]$GithubPath = "",
    [string]$Triplet = "x64-windows",
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
$binDir = Join-Path $installed "bin"
# Las bibliotecas de importación llevan versión en el nombre (tesseract55.lib, leptonica-1.87.0.lib): se buscan.
$tess = Get-ChildItem $libDir -Filter "tesseract*.lib" | Select-Object -First 1
$lept = Get-ChildItem $libDir -Filter "leptonica*.lib" | Select-Object -First 1
if (-not $tess -or -not $lept) { throw "No se encontraron las bibliotecas de Tesseract en $libDir" }

$vars = @{
    TESSERACT_LIB_DIR  = $libDir
    TESSERACT_LIB_NAME = $tess.BaseName
    LEPTONICA_LIB_NAME = $lept.BaseName
}
foreach ($k in $vars.Keys) {
    if ($GithubEnv) { "$k=$($vars[$k])" | Out-File -FilePath $GithubEnv -Append -Encoding utf8 }
    else { Set-Item -Path "Env:$k" -Value $vars[$k] }
}
# Las DLL (tesseract55.dll y dependencias) deben estar en el PATH para ejecutar pruebas y el binario.
if ($GithubPath) { $binDir | Out-File -FilePath $GithubPath -Append -Encoding utf8 }
else { $env:PATH = "$binDir;$env:PATH" }

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
