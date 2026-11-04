# Mide comparar y optimizar en acr12 (W08). Uso, con el turno cogido:
#   . .\tools\acr12-env.ps1
#   .\tools\acr12-turno.ps1 -Flujo w08 -- powershell -File .\tools\w08-bench.ps1
# Genera los PDF en D:\dev-cache\w08-bench (no se suben al repo) y escribe w08-bench.md con tiempos y memoria pico.
$ErrorActionPreference = 'Stop'
$dir = 'D:\dev-cache\w08-bench'
New-Item -ItemType Directory -Force $dir | Out-Null
if (-not (Test-Path "$dir\scan.pdf")) { python tools\gen_w08_bench.py $dir }
cargo build --release -p pliego-cli
$exe = Join-Path $env:CARGO_TARGET_DIR 'release\pliego.exe'
$rows = @()
function Medir($nombre, $argv, $salida) {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $p = Start-Process $exe -ArgumentList $argv -PassThru -NoNewWindow -Wait:$false
    $pico = 0
    while (-not $p.HasExited) { $p.Refresh(); $pico = [Math]::Max($pico, $p.PeakWorkingSet64); Start-Sleep -Milliseconds 50 }
    $sw.Stop()
    $tam = if ($salida -and (Test-Path $salida)) { [Math]::Round((Get-Item $salida).Length / 1MB, 1) } else { '' }
    $script:rows += "| $nombre | {0:N1} s | {1:N0} MB | $tam MB | $($p.ExitCode) |" -f $sw.Elapsed.TotalSeconds, ($pico / 1MB)
}
Medir 'compare texto+visual' "compare $dir\text_a.pdf $dir\text_b.pdf -o $dir\informe.pdf" "$dir\informe.pdf"
Medir 'compare --no-visual' "compare $dir\text_a.pdf $dir\text_b.pdf --no-visual --json -o $dir\informe.json" "$dir\informe.json"
Medir 'optimize --analyze' "optimize $dir\scan.pdf --analyze" $null
foreach ($p in 'pantalla','ebook','impresion','sin-perdida') {
    Medir "optimize --preset $p" "optimize $dir\scan.pdf -o $dir\scan-$p.pdf --preset $p" "$dir\scan-$p.pdf"
}
$md = "| Caso | Tiempo | Memoria pico | Salida | Código |`n| --- | --- | --- | --- | --- |`n" + ($rows -join "`n")
$md | Set-Content "$dir\w08-bench.md" -Encoding utf8
$md
Write-Host "PDF optimizados en $dir\scan-*.pdf: abrirlos en Reader (Ander debe aceptar el control de pantalla)."
