# Mide combinar 50 PDFs de 20 páginas (criterio W04: < 5 s) y genera muestras con Bates y marca de agua para
# abrir en Acrobat Reader. Uso (por turno, con pliego.exe release ya compilado):
#   . .\tools\acr12-env.ps1
#   .\tools\acr12-turno.ps1 -Flujo w04 -- powershell -File tools\bench-w04.ps1
param([string]$Pliego = "$env:CARGO_TARGET_DIR\release\pliego.exe", [string]$Salida = "D:\dev-cache\w04-bench")
$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Force $Salida | Out-Null
python tools\gen_w04_bench.py $Salida
$entradas = Get-ChildItem "$Salida\in\*.pdf" | Sort-Object Name | ForEach-Object FullName
foreach ($n in 1..3) {
    $t = Measure-Command { & $Pliego merge @entradas -o "$Salida\merged.pdf" }
    "merge 50x20 (corrida $n): {0:N2} s" -f $t.TotalSeconds
}
& $Pliego info "$Salida\merged.pdf"
& $Pliego stamp bates "$Salida\merged.pdf" -o "$Salida\muestra-bates.pdf" --prefix "PLG-" --digits 6 --pages 1-5
& $Pliego stamp watermark "$Salida\merged.pdf" -o "$Salida\muestra-marca.pdf" --text "CONFIDENCIAL" --pages 1-5
"Abre en Reader: $Salida\muestra-bates.pdf y $Salida\muestra-marca.pdf"
