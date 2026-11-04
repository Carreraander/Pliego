<#
.SYNOPSIS
  Mide arranque y memoria de pdfium-spike sobre un PDF (pico de conjunto de trabajo y privada, desde fuera).

.EXAMPLE
  . .\tools\acr12-env.ps1
  .\tools\acr12-turno.ps1 -Flujo w35 -- powershell -File .\tools\pdfium\measure.ps1 -Dll D:\dev-cache\pdfium\pdfium.dll -Pdf tests\corpus\basic\x.pdf
#>
[CmdletBinding()]
param([Parameter(Mandatory)][string]$Dll, [Parameter(Mandatory)][string]$Pdf, [int]$Ppp = 150, [int]$Repeticiones = 5)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$exe = Join-Path $repo 'spikes\pdfium-spike\target\release\pdfium-spike.exe'
if (-not (Test-Path $exe)) { throw "Compila antes: cargo build --release --manifest-path spikes\pdfium-spike\Cargo.toml" }
1..$Repeticiones | ForEach-Object {
  $psi = [System.Diagnostics.ProcessStartInfo]::new($exe, "`"$Dll`" `"$Pdf`" $Ppp")
  $psi.RedirectStandardOutput = $true; $psi.UseShellExecute = $false
  $sw = [Diagnostics.Stopwatch]::StartNew()
  $p = [System.Diagnostics.Process]::Start($psi)
  $peakWs = 0; $peakPriv = 0
  while (-not $p.HasExited) {
    try { $p.Refresh(); $peakWs = [math]::Max($peakWs, $p.WorkingSet64); $peakPriv = [math]::Max($peakPriv, $p.PrivateMemorySize64) } catch {}
    Start-Sleep -Milliseconds 5
  }
  $json = $p.StandardOutput.ReadToEnd().Trim()
  [pscustomobject]@{ total_ms = $sw.ElapsedMilliseconds; pico_ws_mb = [math]::Round($peakWs / 1MB, 1); pico_privada_mb = [math]::Round($peakPriv / 1MB, 1); spike = $json }
} | Format-List
