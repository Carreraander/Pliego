<#
.SYNOPSIS
  Prepara (o verifica) las herramientas de desarrollo de Pliego en el equipo de pruebas acr12.

.DESCRIPTION
  Idempotente: si una herramienta ya está instalada, solo la verifica. Instala con winget:
  Git, Rust (rustup, toolchain estable MSVC), Visual Studio Build Tools (C++), LLVM, Node LTS, WiX 3 (MSI)
  y cargo-deny. No toca carpetas de otros flujos: solo crea C:\Users\acr12\dev si no existe.

  Uso:
    powershell -ExecutionPolicy Bypass -File tools\setup-acr12.ps1            # instala lo que falte
    powershell -ExecutionPolicy Bypass -File tools\setup-acr12.ps1 -CheckOnly # solo verifica
    powershell -ExecutionPolicy Bypass -File tools\setup-acr12.ps1 -Report C:\ruta\informe.md

  Código de salida: 0 si todo está disponible al final, 1 si falta algo.
#>
[CmdletBinding()]
param(
    [switch]$CheckOnly,
    [string]$DevRoot = (Join-Path $env:USERPROFILE 'dev'),
    [string]$Report
)

$ErrorActionPreference = 'Continue'
$results = [System.Collections.Generic.List[object]]::new()

function Add-Result($Name, $Status, $Version) {
    $results.Add([pscustomobject]@{ Herramienta = $Name; Estado = $Status; Version = $Version })
    Write-Host ("{0,-22} {1,-12} {2}" -f $Name, $Status, $Version)
}

function Update-SessionPath {
    # Recarga PATH de máquina+usuario y añade rutas habituales para ver lo recién instalado.
    $machine = [Environment]::GetEnvironmentVariable('Path', 'Machine')
    $user = [Environment]::GetEnvironmentVariable('Path', 'User')
    $extra = @("$env:USERPROFILE\.cargo\bin", "$env:ProgramFiles\LLVM\bin", "$env:ProgramFiles\nodejs",
        "$env:ProgramFiles\Git\cmd", "${env:ProgramFiles(x86)}\WiX Toolset v3.14\bin", "${env:ProgramFiles(x86)}\WiX Toolset v3.11\bin")
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
    Write-Host "  -> winget install $Id"
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
Write-Host "Equipo: $env:COMPUTERNAME  Usuario: $env:USERNAME  Windows: $($os.Caption) $($os.Version)"
if (-not $CheckOnly -and -not (Test-Path $DevRoot)) { New-Item -ItemType Directory -Path $DevRoot | Out-Null }

# --- Git, Node, LLVM, WiX -------------------------------------------------------------------------------
Ensure-Tool 'Git' 'git' @('--version') 'Git.Git'
Ensure-Tool 'Node LTS' 'node' @('--version') 'OpenJS.NodeJS.LTS'
Ensure-Tool 'LLVM (clang)' 'clang' @('--version') 'LLVM.LLVM'
Ensure-Tool 'WiX Toolset' 'candle' @('-?') 'WiXToolset.WiXToolset'

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
Add-Result 'cargo' $(if (Get-Command cargo -ErrorAction SilentlyContinue) { 'ok' } else { 'FALTA' }) (Get-ToolVersion 'cargo' @('--version'))

# --- cargo-deny -----------------------------------------------------------------------------------------
$dv = Get-ToolVersion 'cargo-deny' @('--version')
if (-not $dv -and -not $CheckOnly -and (Get-Command cargo -ErrorAction SilentlyContinue)) {
    cargo install cargo-deny --locked | Out-Host
    Update-SessionPath
    $dv = Get-ToolVersion 'cargo-deny' @('--version')
    Add-Result 'cargo-deny' $(if ($dv) { 'instalado' } else { 'FALTA' }) $dv
}
else { Add-Result 'cargo-deny' $(if ($dv) { 'ya estaba' } else { 'FALTA' }) $dv }

# --- Informe --------------------------------------------------------------------------------------------
$missing = @($results | Where-Object { $_.Estado -eq 'FALTA' })
if ($Report) {
    $lines = @("# Preparación de acr12", "", "- Equipo: $env:COMPUTERNAME ($env:USERNAME)",
        "- Windows: $($os.Caption) $($os.Version)", "- Fecha: $(Get-Date -Format 's')", "",
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
