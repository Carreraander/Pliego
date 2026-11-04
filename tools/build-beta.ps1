<#
.SYNOPSIS
  Monta de principio a fin el instalador BETA de Pliego (Pliego-Setup-<versión>-beta.exe) y una actualización de
  prueba firmada (0.1.1) para el E2E de la autoactualización.

.DESCRIPTION
  1. Prepara (o reutiliza) una clave de firma de PRUEBA en D:\dev-cache\beta-keys\ (nunca en el repo; la de producción
     la genera Ander sin conexión, ver docs/releasing.md). Su pública se compila en la app y en el setup con
     PLIEGO_UPDATE_PUBKEYS.
  2. Llama a installer\setup\build.ps1: tauri build de la app, release del worker, la CLI, el lanzador y pliego-setup
     (que es también Uninstall.exe), .paq e instalador.
  3. Copia el instalador como Pliego-Setup-<versión>-beta.exe con su SHA-256.
  4. Construye un .paq de <UpdateVersion> (misma app, solo cambia la versión del paquete) y lo publica firmado con la
     clave de prueba con `pliego-release publish` en <OutDir>\actualizaciones, listo para `pliego-release serve`.

  Es pesado: en acr12 se lanza por turno, con el entorno cargado:
    . .\tools\acr12-env.ps1
    .\tools\acr12-turno.ps1 -Flujo w21 -- powershell -NoProfile -File tools\build-beta.ps1

.PARAMETER Version        Versión del instalador (por defecto, la del workspace).
.PARAMETER UpdateVersion  Versión de la actualización de prueba (por defecto 0.1.1; debe ser posterior a Version).
.PARAMETER OutDir         Salida (por defecto D:\dev\pliego-beta).
.PARAMETER KeyDir         Claves de prueba (por defecto D:\dev-cache\beta-keys).
.PARAMETER UpdateUrl      URL de actualizaciones compilada (por defecto el servidor local de `pliego-release serve`).
.PARAMETER SkipUpdate     No genera la actualización de prueba.
.PARAMETER SkipCargo      Reutiliza target\release (se pasa a build.ps1).
#>
[CmdletBinding()]
param(
  [string]$Version,
  [string]$UpdateVersion = '0.1.1',
  [string]$OutDir = 'D:\dev\pliego-beta',
  [string]$KeyDir = 'D:\dev-cache\beta-keys',
  [string]$UpdateUrl = 'http://127.0.0.1:8787',
  [switch]$SkipUpdate,
  [switch]$SkipCargo
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
if (-not $Version) {
  $m = Select-String -Path (Join-Path $repo 'Cargo.toml') -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1
  $Version = $m.Matches[0].Groups[1].Value
}
if ([version]$UpdateVersion -le [version]$Version) { throw "UpdateVersion ($UpdateVersion) debe ser posterior a Version ($Version)." }
if ($KeyDir -like "$repo*") { throw 'La clave de prueba no puede estar dentro del repositorio.' }
if (-not $env:CARGO_TARGET_DIR) { Write-Warning 'CARGO_TARGET_DIR no está definido: carga antes `. .\tools\acr12-env.ps1`.' }
$target = if ($env:CARGO_TARGET_DIR) { Join-Path $env:CARGO_TARGET_DIR 'release' } else { Join-Path $repo 'target\release' }

function Invoke-Checked([string]$exe, [string[]]$argv) {
  & $exe @argv | Out-Host
  if ($LASTEXITCODE -ne 0) { throw "$exe terminó con código $LASTEXITCODE" }
}

Push-Location $repo
try {
  # 1. Clave de PRUEBA (el nombre lo deja claro) y herramienta de publicación.
  if (-not $SkipCargo) { Invoke-Checked 'cargo' @('build', '--release', '--locked', '-p', 'pliego-release') }
  $release = Join-Path $target 'pliego-release.exe'
  $keyName = 'pliego-PRUEBA-beta'
  $secret = Join-Path $KeyDir "$keyName.key"
  $pubFile = Join-Path $KeyDir "$keyName.pub"
  if (-not (Test-Path $secret)) { Invoke-Checked $release @('keygen', '--out', $KeyDir, '--name', $keyName) }
  $pub = (Get-Content $pubFile -Raw).Trim()
  Write-Host "Clave de PRUEBA: $pub (NO es la de producción)"

  # Se compilan en la app y en el setup (build.ps1 hereda estas variables).
  $env:PLIEGO_UPDATE_PUBKEYS = $pub
  $env:PLIEGO_UPDATE_URL = $UpdateUrl

  # 2. Instalador y paquete.
  $dist = Join-Path $OutDir 'dist'
  $argsBuild = @('-Version', $Version, '-OutDir', $dist)
  if ($SkipCargo) { $argsBuild += '-SkipCargo' }
  & (Join-Path $repo 'installer\setup\build.ps1') @argsBuild
  if ($LASTEXITCODE -ne 0 -and $null -ne $LASTEXITCODE) { throw "build.ps1 terminó con código $LASTEXITCODE" }

  # 3. Instalador beta + SHA-256.
  $built = Join-Path $dist "Pliego-Setup-$Version-x86_64.exe"
  $beta = Join-Path $OutDir "Pliego-Setup-$Version-beta.exe"
  Copy-Item $built $beta -Force
  $hash = (Get-FileHash $beta -Algorithm SHA256).Hash.ToLower()
  "$hash  $(Split-Path $beta -Leaf)" | Set-Content "$beta.sha256" -Encoding ascii
  Write-Host "Instalador beta: $beta ($([math]::Round((Get-Item $beta).Length / 1MB, 1)) MB)`nSHA-256: $hash"

  # 4. Actualización de prueba firmada.
  if (-not $SkipUpdate) {
    # Misma app con otra versión de paquete: se parte de la carpeta ya montada, sin lo que genera build.ps1.
    $appCopy = Join-Path $OutDir 'stage-update-app'
    if (Test-Path $appCopy) { Remove-Item $appCopy -Recurse -Force }
    Copy-Item (Join-Path $dist "stage\app-$Version") $appCopy -Recurse
    foreach ($n in 'files.json', 'launcher', 'pliego-setup.exe', 'LICENSE') {
      Remove-Item (Join-Path $appCopy $n) -Recurse -Force -ErrorAction SilentlyContinue
    }
    $distUpd = Join-Path $OutDir 'dist-update'
    & (Join-Path $repo 'installer\setup\build.ps1') -Version $UpdateVersion -OutDir $distUpd -AppDir $appCopy -SkipCargo
    if ($LASTEXITCODE -ne 0 -and $null -ne $LASTEXITCODE) { throw "build.ps1 (actualización) terminó con código $LASTEXITCODE" }

    $tree = Join-Path $OutDir 'actualizaciones'
    Invoke-Checked $release @(
      'publish', '--root', $tree, '--channel', 'stable', '--version', $UpdateVersion,
      '--package', (Join-Path $distUpd "pliego-$UpdateVersion-x86_64.paq"),
      '--setup', (Join-Path $distUpd "Pliego-Setup-$UpdateVersion-x86_64.exe"),
      '--notes-es', 'Versión de PRUEBA para el E2E de autoactualización.',
      '--notes-en', 'TEST release for the self-update E2E.',
      '--rollout', '100', '--key', $secret)
    Invoke-Checked $release @('verify', '--root', $tree, '--channel', 'stable', '--pubkey', $pubFile)
    Remove-Item $appCopy -Recurse -Force
    Write-Host "Actualización de prueba $UpdateVersion publicada en $tree"
    Write-Host "Para servirla: $release serve --root $tree   (escucha en 127.0.0.1:8787)"
  }
} finally { Pop-Location }
