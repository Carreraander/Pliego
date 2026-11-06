<#
.SYNOPSIS
  Valida la impresión de Pliego (W44) en Windows sin gastar papel: imprime con «Microsoft Print to PDF» (que viene con
  Windows 10/11) y comprueba el PDF resultante (páginas, tamaño de hoja) y que sale legible (lo renderiza a PNG).

.DESCRIPTION
  Uso (en acr12, por turno; ver docs/handoff/reglas-operativas.md):
    . .\tools\acr12-env.ps1
    .\tools\acr12-turno.ps1 -Flujo w44 -- powershell -File .\tools\validar-impresion.ps1
  Compila la CLI en debug si no hay `pliego.exe`; deja los resultados en <carpeta>\salida (por defecto
  $env:TEMP\pliego-imprimir) y escribe una línea OK / FALLO por caso. Código de salida 0 solo si todos pasan.
  Las comprobaciones de "qué se ve" (la página cae centrada, sin cortes) son de revisión humana: abre los PNG.

.PARAMETER Impresora
  Impresora de pruebas. Por defecto «Microsoft Print to PDF».

.PARAMETER Pliego
  Ruta de pliego.exe. Por defecto target\debug\pliego.exe del clon (o $env:CARGO_TARGET_DIR\debug\pliego.exe).

.PARAMETER Salida
  Carpeta donde dejar los PDF y los PNG.
#>
param(
    [string]$Impresora = 'Microsoft Print to PDF',
    [string]$Pliego = '',
    [string]$Salida = (Join-Path $env:TEMP 'pliego-imprimir')
)
$ErrorActionPreference = 'Stop'
$raiz = Split-Path $PSScriptRoot -Parent
if (-not $Pliego) {
    $objetivo = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $raiz 'target' }
    $Pliego = Join-Path $objetivo 'debug\pliego.exe'
    if (-not (Test-Path $Pliego)) {
        Push-Location $raiz; try { cargo build -p pliego-cli } finally { Pop-Location }
    }
}
if (-not (Test-Path $Pliego)) { throw "No encuentro pliego.exe ($Pliego)" }
New-Item -ItemType Directory -Force -Path $Salida | Out-Null
Get-ChildItem $Salida -ErrorAction SilentlyContinue | Remove-Item -Recurse -Force

$corpus = Join-Path $raiz 'tests\corpus'
$hola = Join-Path $corpus 'basic\hola.pdf'
$fallos = 0

function Caso([string]$nombre, [scriptblock]$cuerpo) {
    try {
        & $cuerpo
        Write-Host "OK     $nombre"
    } catch {
        $script:fallos++
        Write-Host "FALLO  $nombre : $($_.Exception.Message)"
    }
}

function Imprimir([string]$entrada, [string]$destino, [string[]]$extra) {
    if (Test-Path $destino) { Remove-Item $destino -Force }
    $argumentos = @('--json', 'printer', 'send', $entrada, '-P', $Impresora, '-o', $destino) + $extra
    $texto = & $Pliego @argumentos 2>$null
    if ($LASTEXITCODE -ne 0) { throw "pliego printer send terminó con $LASTEXITCODE" }
    # «Microsoft Print to PDF» escribe el archivo al cerrar el trabajo; espera un poco.
    $limite = (Get-Date).AddSeconds(30)
    while (-not (Test-Path $destino) -and (Get-Date) -lt $limite) { Start-Sleep -Milliseconds 300 }
    if (-not (Test-Path $destino)) { throw "no apareció $destino" }
    Start-Sleep -Milliseconds 500
    return ($texto | Out-String | ConvertFrom-Json)
}

function Info([string]$pdf) { (& $Pliego --json info $pdf | Out-String | ConvertFrom-Json) }

function Pagina([string]$pdf, [string]$prefijo) {
    $dir = Join-Path $Salida $prefijo
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    & $Pliego render $pdf --pages 1 --dpi 60 -o $dir | Out-Null
}

Caso 'lista las impresoras y encuentra la de pruebas' {
    $lista = & $Pliego --json printer list | Out-String | ConvertFrom-Json
    if (-not ($lista | Where-Object { $_.name -eq $Impresora })) { throw "no está «$Impresora»" }
}
Caso 'capacidades: da papeles y A4 entre ellos' {
    $c = & $Pliego --json printer caps $Impresora | Out-String | ConvertFrom-Json
    if ($c.papers.Count -lt 1) { throw 'sin papeles' }
    if (-not ($c.papers | Where-Object { $_.id -eq 9 })) { throw 'no declara A4 (id 9)' }
}
Caso 'zona imprimible en puntos de una hoja A4' {
    $s = & $Pliego --json printer sheet $Impresora --paper 9 | Out-String | ConvertFrom-Json
    if ([math]::Abs($s.physW - 595) -gt 3 -or [math]::Abs($s.physH - 842) -gt 3) { throw "hoja inesperada $($s.physW)x$($s.physH)" }
}
Caso 'una página, ajustada, sale como 1 hoja A4' {
    $r = Imprimir $hola (Join-Path $Salida 'ajustar.pdf') @('--pages', '1', '--paper', '9')
    $i = Info (Join-Path $Salida 'ajustar.pdf')
    if ($i.page_count -ne 1) { throw "páginas: $($i.page_count)" }
    Pagina (Join-Path $Salida 'ajustar.pdf') 'ajustar'
}
Caso 'tamaño real y porcentaje propio' {
    Imprimir $hola (Join-Path $Salida 'real.pdf') @('--pages', '1', '--paper', '9', '--scale', 'actual') | Out-Null
    Imprimir $hola (Join-Path $Salida 'mitad.pdf') @('--pages', '1', '--paper', '9', '--scale', 'custom', '--scale-percent', '50') | Out-Null
    Pagina (Join-Path $Salida 'real.pdf') 'real'
    Pagina (Join-Path $Salida 'mitad.pdf') 'mitad'
}
Caso 'hoja apaisada y escala de grises' {
    Imprimir $hola (Join-Path $Salida 'apaisada.pdf') @('--pages', '1', '--paper', '9', '--orientation', 'landscape') | Out-Null
    Imprimir $hola (Join-Path $Salida 'grises.pdf') @('--pages', '1', '--paper', '9', '--grayscale') | Out-Null
    $i = Info (Join-Path $Salida 'apaisada.pdf')
    if ($i.page_count -ne 1) { throw "páginas: $($i.page_count)" }
    Pagina (Join-Path $Salida 'apaisada.pdf') 'apaisada'
    Pagina (Join-Path $Salida 'grises.pdf') 'grises'
}
Caso 'rechaza opciones no válidas antes de tocar la impresora' {
    & $Pliego printer send $hola -P $Impresora --copies 0 2>$null
    if ($LASTEXITCODE -eq 0) { throw 'aceptó 0 copias' }
    & $Pliego printer send $hola -P $Impresora --pages 99 2>$null
    if ($LASTEXITCODE -eq 0) { throw 'aceptó una página inexistente' }
}
$varias = Get-ChildItem (Join-Path $corpus '*\*.pdf') -ErrorAction SilentlyContinue |
    Where-Object { (Info $_.FullName).page_count -ge 3 } | Select-Object -First 1
if ($varias) {
    Caso "varias páginas ($($varias.Name)): un intervalo, solo pares y al revés" {
        $total = (Info $varias.FullName).page_count
        Imprimir $varias.FullName (Join-Path $Salida 'rango.pdf') @('--paper', '9', '--pages', '2-3') | Out-Null
        if ((Info (Join-Path $Salida 'rango.pdf')).page_count -ne 2) { throw 'el intervalo 2-3 no dio 2 hojas' }
        Imprimir $varias.FullName (Join-Path $Salida 'pares.pdf') @('--paper', '9', '--subset', 'even') | Out-Null
        if ((Info (Join-Path $Salida 'pares.pdf')).page_count -ne [math]::Floor($total / 2)) { throw 'las pares no cuadran' }
        Imprimir $varias.FullName (Join-Path $Salida 'alreves.pdf') @('--paper', '9', '--reverse') | Out-Null
        if ((Info (Join-Path $Salida 'alreves.pdf')).page_count -ne $total) { throw 'al revés no imprimió todas' }
    }
}
Write-Host ''
Write-Host "Resultados en $Salida (abre los PNG de cada subcarpeta para el juicio visual)."
if ($fallos -gt 0) { Write-Host "$fallos caso(s) fallaron."; exit 1 }
Write-Host 'Todos los casos pasan.'
