<#
.SYNOPSIS
  Benchmarks de rendimiento de Pliego en Windows (acr12). Escribe un resultado por versión en docs/qa/bench/.
.DESCRIPTION
  Uso:  . .\tools\acr12-env.ps1 ; .\tools\bench.ps1
  Genera el PDF de 500 páginas, ejecuta `bench` (apertura, tesela 512 px, página) y mide la memoria máxima del proceso.
  Objetivos: abrir 500 páginas < 1,5 s; tesela 512 px < 30 ms de media; memoria máxima razonable (< 150 MB en reposo).
#>
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
Set-Location $repo
$pdf = Join-Path $repo 'tests\output\bench-500.pdf'
if (-not (Test-Path $pdf)) { python tools\gen_bench_pdf.py --pages 500 --out $pdf }
cargo build --release -p pliego-qa --bin bench
$target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $repo 'target' }
$exe = Join-Path $target 'release\bench.exe'

$p = Start-Process -FilePath $exe -ArgumentList @($pdf, '--json') -NoNewWindow -PassThru -RedirectStandardOutput "$env:TEMP\bench-out.json"
$peak = 0
while (-not $p.HasExited) {
    try { $p.Refresh(); if ($p.PeakWorkingSet64 -gt $peak) { $peak = $p.PeakWorkingSet64 } } catch {}
    Start-Sleep -Milliseconds 20
}
$p.WaitForExit()
$result = Get-Content "$env:TEMP\bench-out.json" -Raw | ConvertFrom-Json
$version = (cargo metadata --no-deps --format-version 1 | ConvertFrom-Json).packages | Where-Object name -eq 'pliego-cli' | Select-Object -First 1 -ExpandProperty version
$commit = (git rev-parse --short HEAD)
$out = [ordered]@{
    fecha = (Get-Date -Format 's'); equipo = $env:COMPUTERNAME; windows = (Get-CimInstance Win32_OperatingSystem).Version
    version = $version; commit = $commit; pico_memoria_mb = [math]::Round($peak / 1MB, 1)
    resultado = $result
}
New-Item -ItemType Directory -Force -Path docs\qa\bench | Out-Null
$file = "docs\qa\bench\$version-$commit.json"
$out | ConvertTo-Json -Depth 4 | Set-Content -Path $file -Encoding UTF8
& $exe $pdf
Write-Host "Memoria máxima del proceso: $($out.pico_memoria_mb) MB`nResultado guardado en $file"
