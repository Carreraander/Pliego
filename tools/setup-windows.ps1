<#
.SYNOPSIS
  Prepara (o verifica) las herramientas de desarrollo de Pliego en un equipo Windows de pruebas (acr12, pc...).

.DESCRIPTION
  Idempotente: si una herramienta ya está instalada, solo la verifica. Instala con winget, cargo o descargas:
  Git, Node LTS, LLVM, LLVM 19 (para el bindgen de tesseract-sys), Visual Studio Build Tools (C++), Rust (MSVC), WiX y cargo-deny.
  Con -ConTesseract además Tesseract/Leptonica de vcpkg; con -ConExtras Acrobat Reader, Ghostscript y veraPDF (con
  un JRE). Nunca instala Office. Todo lo grande va al disco de trabajo (-Disco): por defecto D: si existe, si no C:.
  No toca carpetas de otros flujos ni de otros proyectos: solo crea <disco>\dev, <disco>\dev-tools y <disco>\dev-cache.

  Uso:
    powershell -ExecutionPolicy Bypass -File tools\setup-windows.ps1                 # instala lo básico que falte
    powershell -ExecutionPolicy Bypass -File tools\setup-windows.ps1 -CheckOnly      # solo verifica
    powershell -ExecutionPolicy Bypass -File tools\setup-windows.ps1 -ConTesseract -ConExtras -Report informe.md
  En acr12 se usa el envoltorio tools\setup-acr12.ps1; en pc, ver docs/qa/pc.md.

  Código de salida: 0 si todo lo pedido está disponible al final, 1 si falta algo.
#>
[CmdletBinding()]
param(
    [switch]$CheckOnly,
    [string]$Disco = $(if (Test-Path 'D:\') { 'D:' } else { 'C:' }),
    [string]$DevRoot,
    [string]$ToolsRoot,
    [string]$Llvm19Dir,
    [switch]$SinLlvm19,      # no instala LLVM 19 (p. ej. sin espacio): pliego-env.ps1 usa entonces el LLVM del sistema
    [switch]$ConTesseract,   # Tesseract y Leptonica de vcpkg (necesarios para compilar pliego-cli y la app)
    [switch]$ConExtras,      # Acrobat Reader gratuito, Ghostscript y veraPDF (con JRE)
    [string]$Report
)

$ErrorActionPreference = 'Continue'
$Disco = $Disco.TrimEnd('\')
if (-not $DevRoot) { $DevRoot = "$Disco\dev" }
if (-not $ToolsRoot) { $ToolsRoot = "$Disco\dev-tools" }
if (-not $Llvm19Dir) { $Llvm19Dir = if ($Disco -eq 'D:') { 'D:\LLVM19' } else { "$ToolsRoot\LLVM19" } }
$cacheRoot = "$Disco\dev-cache"
$results = [System.Collections.Generic.List[object]]::new()

function Add-Result($Name, $Status, $Version) {
    $results.Add([pscustomobject]@{ Herramienta = $Name; Estado = $Status; Version = $Version })
    Write-Host ("{0,-22} {1,-12} {2}" -f $Name, $Status, $Version)
}

function Update-SessionPath {
    # Recarga PATH de máquina+usuario y añade rutas habituales para ver lo recién instalado.
    $machine = [Environment]::GetEnvironmentVariable('Path', 'Machine')
    $user = [Environment]::GetEnvironmentVariable('Path', 'User')
    $cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { "$env:USERPROFILE\.cargo" }
    $extra = @("$cargoHome\bin", "$env:ProgramFiles\LLVM\bin", "$env:ProgramFiles\nodejs", "$env:ProgramFiles\Git\cmd",
        "${env:ProgramFiles(x86)}\WiX Toolset v3.14\bin", "${env:ProgramFiles(x86)}\WiX Toolset v3.11\bin", "$env:USERPROFILE\.dotnet\tools")
    $env:Path = (($machine, $user) + $extra | Where-Object { $_ }) -join ';'
}

function Get-ToolVersion($Cmd, $VArgs) {
    $c = Get-Command $Cmd -ErrorAction SilentlyContinue
    if (-not $c) { return $null }
    try { return ((& $c.Source @VArgs 2>&1 | Select-Object -First 1) -as [string]).Trim() } catch { return $null }
}

function Install-Winget($Id, $ExtraArgs = @()) {
    if ($CheckOnly) { return $false }
    if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
        Write-Warning 'winget no está disponible; instala "App Installer" desde Microsoft Store.'
        return $false
    }
    Write-Host "  -> winget install $Id $ExtraArgs"
    winget install --id $Id -e --accept-source-agreements --accept-package-agreements --silent @ExtraArgs | Out-Host
    Update-SessionPath
    return $true
}

function Ensure-Tool($Name, $Cmd, $VersionArgs, $WingetId, $ExtraArgs = @()) {
    $v = Get-ToolVersion $Cmd $VersionArgs
    if ($v) { Add-Result $Name 'ya estaba' $v; return }
    if (Install-Winget $WingetId $ExtraArgs) {
        $v = Get-ToolVersion $Cmd $VersionArgs
        if ($v) { Add-Result $Name 'instalado' $v; return }
    }
    Add-Result $Name 'FALTA' ''
}

Update-SessionPath
$os = (Get-CimInstance Win32_OperatingSystem)
$libre = [math]::Round((Get-PSDrive $Disco.TrimEnd(':')).Free / 1GB, 1)
Write-Host "Equipo: $env:COMPUTERNAME  Usuario: $env:USERNAME  Windows: $($os.Caption) $($os.Version)  Disco: $Disco ($libre GB libres)"
if (-not $CheckOnly) { foreach ($d in $DevRoot, $ToolsRoot, $cacheRoot) { New-Item -ItemType Directory -Force -Path $d | Out-Null } }

# --- Git, Node, LLVM, WiX -------------------------------------------------------------------------------
Ensure-Tool 'Git' 'git' @('--version') 'Git.Git'
Ensure-Tool 'Node LTS' 'node' @('--version') 'OpenJS.NodeJS.LTS'
Ensure-Tool 'LLVM (clang)' 'clang' @('--version') 'LLVM.LLVM'
# Tauri descarga su propio WiX 3 al empaquetar; aquí vale WiX 3 (candle) o WiX 4/5 (wix, herramienta dotnet).
$wv = Get-ToolVersion 'candle' @('-?')
if (-not $wv) { $wv = Get-ToolVersion 'wix' @('--version') }
if ($wv) { Add-Result 'WiX Toolset' 'ya estaba' $wv } else { Ensure-Tool 'WiX Toolset' 'candle' @('-?') 'WiXToolset.WiXToolset' }

# --- LLVM 19 en paralelo (para el bindgen de tesseract-sys) -------------------------------------------------------------
# Como el runner de GitHub. No toca el LLVM instalado ni variables globales: pliego-env.ps1 lo pone en el PATH
# solo de la terminal. Lo imprescindible es tener clang.exe en el PATH (sin él falta max_align_t).
if (Test-Path "$Llvm19Dir\bin\libclang.dll") { Add-Result 'LLVM 19 (libclang)' 'ya estaba' $Llvm19Dir }
elseif ($SinLlvm19) { Add-Result 'LLVM 19 (libclang)' 'omitido' 'se usa el LLVM del sistema (-SinLlvm19)' }
elseif (-not $CheckOnly) {
    Install-Winget 'LLVM.LLVM' @('--version', '19.1.7', '--location', $Llvm19Dir, '--force') | Out-Null
    Add-Result 'LLVM 19 (libclang)' $(if (Test-Path "$Llvm19Dir\bin\libclang.dll") { 'instalado' } else { 'FALTA' }) $Llvm19Dir
}
else { Add-Result 'LLVM 19 (libclang)' 'FALTA' '' }

# --- Visual Studio Build Tools (C++) --------------------------------------------------------------------
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
function Get-VcTools {
    if (-not (Test-Path $vswhere)) { return $null }
    $p = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationVersion 2>$null
    if ($p) { return "VS $p" }
    return $null
}
$vc = Get-VcTools
if ($vc) { Add-Result 'VS Build Tools C++' 'ya estaba' $vc }
elseif (-not $CheckOnly) {
    Install-Winget 'Microsoft.VisualStudio.2022.BuildTools' @('--override',
        '--wait --quiet --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended') | Out-Null
    $vc = Get-VcTools
    Add-Result 'VS Build Tools C++' $(if ($vc) { 'instalado' } else { 'FALTA' }) $vc
}
else { Add-Result 'VS Build Tools C++' 'FALTA' '' }

# --- Rust -----------------------------------------------------------------------------------------------
$rv = Get-ToolVersion 'rustc' @('--version')
if (-not $rv -and -not $CheckOnly) {
    Install-Winget 'Rustlang.Rustup' | Out-Null
    if (Get-Command rustup -ErrorAction SilentlyContinue) {
        rustup default stable-x86_64-pc-windows-msvc | Out-Host
        rustup component add rustfmt clippy | Out-Host
    }
    $rv = Get-ToolVersion 'rustc' @('--version')
    Add-Result 'Rust (rustc)' $(if ($rv) { 'instalado' } else { 'FALTA' }) $rv
}
elseif ($rv) {
    if (-not $CheckOnly) { rustup component add rustfmt clippy 2>&1 | Out-Null }
    Add-Result 'Rust (rustc)' 'ya estaba' $rv
}
else { Add-Result 'Rust (rustc)' 'FALTA' '' }
Add-Result 'cargo' $(if (Get-Command cargo -ErrorAction SilentlyContinue) { 'ya estaba' } else { 'FALTA' }) (Get-ToolVersion 'cargo' @('--version'))

# --- cargo-deny -----------------------------------------------------------------------------------------
# Binario publicado (segundos y sin target de 1 GB); si falla, se compila con cargo install (~6 min).
$dv = Get-ToolVersion 'cargo-deny' @('--version')
if (-not $dv -and -not $CheckOnly -and (Get-Command cargo -ErrorAction SilentlyContinue)) {
    $cargoBin = Join-Path $(if ($env:CARGO_HOME) { $env:CARGO_HOME } else { "$env:USERPROFILE\.cargo" }) 'bin'
    try {
        $rel = Invoke-RestMethod 'https://api.github.com/repos/EmbarkStudios/cargo-deny/releases/latest'
        $asset = $rel.assets | Where-Object name -like '*x86_64-pc-windows-msvc.tar.gz' | Select-Object -First 1
        $tmp = Join-Path $cacheRoot 'tmp\cargo-deny'
        New-Item -ItemType Directory -Force -Path $tmp | Out-Null
        Invoke-WebRequest $asset.browser_download_url -OutFile "$tmp\cargo-deny.tar.gz"
        tar -xzf "$tmp\cargo-deny.tar.gz" -C $tmp
        Copy-Item (Get-ChildItem $tmp -Recurse -Filter cargo-deny.exe | Select-Object -First 1).FullName $cargoBin
        Remove-Item -Recurse -Force $tmp
    } catch { cargo install cargo-deny --locked | Out-Host }
    Update-SessionPath
    $dv = Get-ToolVersion 'cargo-deny' @('--version')
    Add-Result 'cargo-deny' $(if ($dv) { 'instalado' } else { 'FALTA' }) $dv
}
else { Add-Result 'cargo-deny' $(if ($dv) { 'ya estaba' } else { 'FALTA' }) $dv }

# --- Tesseract y Leptonica (vcpkg) ----------------------------------------------------------------------
if ($ConTesseract) {
    $vcpkg = @($env:VCPKG_ROOT, "$env:USERPROFILE\vcpkg", "$ToolsRoot\vcpkg", 'C:\vcpkg') | Where-Object { $_ -and (Test-Path "$_\vcpkg.exe") } | Select-Object -First 1
    $hayTess = { param($r) $r -and (Get-ChildItem "$r\installed\x64-windows-release\lib" -Filter 'tesseract*.lib' -ErrorAction SilentlyContinue) }
    if (& $hayTess $vcpkg) { Add-Result 'Tesseract (vcpkg)' 'ya estaba' "$vcpkg\installed\x64-windows-release" }
    elseif (-not $CheckOnly) {
        if (-not $vcpkg) {
            $vcpkg = "$ToolsRoot\vcpkg"
            git clone --depth 1 https://github.com/microsoft/vcpkg $vcpkg | Out-Host
            & "$vcpkg\bootstrap-vcpkg.bat" -disableMetrics | Out-Host
        }
        foreach ($d in 'vcpkg-binary', 'vcpkg-downloads') { New-Item -ItemType Directory -Force -Path "$cacheRoot\$d" | Out-Null }
        $env:VCPKG_DEFAULT_BINARY_CACHE = "$cacheRoot\vcpkg-binary"
        $env:VCPKG_DOWNLOADS = "$cacheRoot\vcpkg-downloads"
        # --clean-after-build: borra buildtrees y paquetes intermedios (varios GB) en cuanto acaba cada puerto.
        & "$vcpkg\vcpkg.exe" install tesseract:x64-windows-release leptonica:x64-windows-release --clean-after-build | Out-Host
        Add-Result 'Tesseract (vcpkg)' $(if (& $hayTess $vcpkg) { 'instalado' } else { 'FALTA' }) "$vcpkg\installed\x64-windows-release"
    }
    else { Add-Result 'Tesseract (vcpkg)' 'FALTA' '' }
}

# --- Extras de prueba: Acrobat Reader, Ghostscript, veraPDF ---------------------------------------------
if ($ConExtras) {
    $reader = @("$env:ProgramFiles\Adobe\Acrobat DC\Acrobat\Acrobat.exe", "${env:ProgramFiles(x86)}\Adobe\Acrobat Reader DC\Reader\AcroRd32.exe",
        "$env:ProgramFiles\Adobe\Acrobat Reader DC\Reader\AcroRd32.exe") | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $reader -and -not $CheckOnly) {
        Install-Winget 'Adobe.Acrobat.Reader.64-bit' | Out-Null
        $reader = "$env:ProgramFiles\Adobe\Acrobat DC\Acrobat\Acrobat.exe"
        Add-Result 'Acrobat Reader' $(if (Test-Path $reader) { 'instalado' } else { 'FALTA' }) $reader
    } else { Add-Result 'Acrobat Reader' $(if ($reader) { 'ya estaba' } else { 'FALTA' }) $reader }

    $gs = Get-ChildItem "$env:ProgramFiles\gs\*\bin\gswin64c.exe" -ErrorAction SilentlyContinue | Select-Object -Last 1
    if (-not $gs -and -not $CheckOnly) {
        # Ghostscript ya no está en winget (2026-11): instalador oficial de Artifex en GitHub, con su firma comprobada.
        # Instala en todo el equipo: pide UAC.
        try {
            $rel = Invoke-RestMethod 'https://api.github.com/repos/ArtifexSoftware/ghostpdl-downloads/releases/latest'
            $asset = $rel.assets | Where-Object name -match '^gs\d+w64\.exe$' | Select-Object -First 1
            $tmp = Join-Path $cacheRoot 'tmp\ghostscript'
            New-Item -ItemType Directory -Force -Path $tmp | Out-Null
            $exe = Join-Path $tmp $asset.name
            Invoke-WebRequest $asset.browser_download_url -OutFile $exe
            $firma = Get-AuthenticodeSignature $exe
            if ($firma.Status -ne 'Valid' -or $firma.SignerCertificate.Subject -notlike '*Artifex Software*') { throw "firma no válida: $($firma.Status)" }
            Start-Process $exe -ArgumentList '/S' -Verb RunAs -Wait
            Remove-Item -Recurse -Force $tmp
        } catch { Write-Warning "No se pudo instalar Ghostscript: $_" }
        $gs = Get-ChildItem "$env:ProgramFiles\gs\*\bin\gswin64c.exe" -ErrorAction SilentlyContinue | Select-Object -Last 1
        Add-Result 'Ghostscript' $(if ($gs) { 'instalado' } else { 'FALTA' }) "$($gs.FullName)"
    } else { Add-Result 'Ghostscript' $(if ($gs) { 'ya estaba' } else { 'FALTA' }) "$($gs.FullName)" }

    # veraPDF: JRE portátil (Temurin 21) e instalación desatendida con IzPack en <ToolsRoot>\veraPDF.
    $jre = Get-ChildItem "$ToolsRoot\jre\*\bin\java.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
    $vera = "$ToolsRoot\veraPDF\verapdf.bat"
    if (-not (Test-Path $vera) -and -not $CheckOnly) {
        $tmp = Join-Path $cacheRoot 'tmp\verapdf'
        New-Item -ItemType Directory -Force -Path $tmp | Out-Null
        if (-not $jre) {
            Invoke-WebRequest 'https://api.adoptium.net/v3/binary/latest/21/ga/windows/x64/jre/hotspot/normal/eclipse' -OutFile "$tmp\jre.zip"
            Expand-Archive "$tmp\jre.zip" "$ToolsRoot\jre" -Force
            $jre = Get-ChildItem "$ToolsRoot\jre\*\bin\java.exe" | Select-Object -First 1
        }
        Invoke-WebRequest 'https://software.verapdf.org/releases/verapdf-installer.zip' -OutFile "$tmp\vera.zip"
        Expand-Archive "$tmp\vera.zip" $tmp -Force
        $jar = Get-ChildItem $tmp -Recurse -Filter 'verapdf-izpack-installer-*.jar' | Select-Object -First 1
        $auto = "$tmp\auto-install.xml"
        @"
<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<AutomatedInstallation langpack="eng">
  <com.izforge.izpack.panels.htmlhello.HTMLHelloPanel id="welcome"/>
  <com.izforge.izpack.panels.target.TargetPanel id="install_dir"><installpath>$ToolsRoot\veraPDF</installpath></com.izforge.izpack.panels.target.TargetPanel>
  <com.izforge.izpack.panels.packs.PacksPanel id="sdk_pack_select">
    <pack index="0" name="veraPDF GUI" selected="true"/>
    <pack index="1" name="veraPDF Mac and *nix Scripts" selected="false"/>
    <pack index="2" name="veraPDF Validation model" selected="false"/>
    <pack index="3" name="veraPDF Documentation" selected="false"/>
    <pack index="4" name="veraPDF Sample Plugins" selected="false"/>
  </com.izforge.izpack.panels.packs.PacksPanel>
  <com.izforge.izpack.panels.install.InstallPanel id="install"/>
  <com.izforge.izpack.panels.finish.FinishPanel id="finish"/>
</AutomatedInstallation>
"@ | Set-Content -Path $auto -Encoding UTF8
        & $jre.FullName -jar $jar.FullName $auto | Out-Host
        Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
        Add-Result 'veraPDF' $(if (Test-Path $vera) { 'instalado' } else { 'FALTA' }) $vera
    } else { Add-Result 'veraPDF' $(if (Test-Path $vera) { 'ya estaba' } else { 'FALTA' }) $vera }
    if ($jre) { Write-Host "  veraPDF necesita Java: JAVA_HOME=$(Split-Path (Split-Path $jre.FullName))  (pliego-env.ps1 lo pone)" }
}

# --- Aviso de saltos de línea ---------------------------------------------------------------------------
# Con core.autocrlf=true, los PDF de tests/corpus pueden salir con CRLF (clones anteriores a `*.pdf -text` en
# .gitattributes) y fallar las pruebas de firma ("object ID 4 0 not found"). Repara el clon con:
#   git rm --cached -r tests/corpus; git checkout -- tests/corpus
if ((Get-Command git -ErrorAction SilentlyContinue) -and ((git config --get core.autocrlf) -eq 'true')) {
    Write-Warning 'core.autocrlf=true: si fallan pruebas con PDF de tests/corpus, ejecuta en tu clon: git rm --cached -r tests/corpus; git checkout -- tests/corpus'
}

# --- Informe --------------------------------------------------------------------------------------------
$missing = @($results | Where-Object { $_.Estado -eq 'FALTA' })
if ($Report) {
    $lines = @("# Preparación de $env:COMPUTERNAME", "", "- Equipo: $env:COMPUTERNAME ($env:USERNAME)",
        "- Windows: $($os.Caption) $($os.Version)", "- Disco de trabajo: $Disco ($libre GB libres al empezar)", "- Fecha: $(Get-Date -Format 's')", "",
        '| Herramienta | Estado | Versión |', '|---|---|---|')
    $lines += $results | ForEach-Object { "| $($_.Herramienta) | $($_.Estado) | $($_.Version) |" }
    Set-Content -Path $Report -Value $lines -Encoding UTF8
    Write-Host "Informe escrito en $Report"
}
if ($missing.Count -gt 0) {
    Write-Host "`nFaltan: $($missing.Herramienta -join ', '). Abre una terminal nueva y repite si acabas de instalar algo." -ForegroundColor Yellow
    exit 1
}
Write-Host "`nTodo listo. Clona tu flujo en $DevRoot\pliego-<flujo> (p. ej. pliego-w10)." -ForegroundColor Green
exit 0
