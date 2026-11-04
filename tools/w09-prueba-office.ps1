# Prueba manual de W09 en acr12: genera un .docx, .xlsx y .pptx ficticios con COM de Office y los convierte
# con `pliego create`. Requiere Office instalado y `pliego.exe` compilado (ruta en -Pliego).
# Uso: . .\tools\acr12-env.ps1; .\tools\w09-prueba-office.ps1 -Pliego D:\dev-cache\target\pliego-w09\debug\pliego.exe
param(
    [Parameter(Mandatory)][string]$Pliego,
    [string]$Salida = 'D:\dev-cache\w09-office'
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $Salida | Out-Null
$ok = $true

function Cerrar($app) { try { $app.Quit() } catch {}; [void][Runtime.InteropServices.Marshal]::ReleaseComObject($app) }

# Word
$w = New-Object -ComObject Word.Application; $w.Visible = $false
try {
    $d = $w.Documents.Add()
    $d.Content.Text = "Informe ficticio de prueba`rSegundo párrafo con acentos: áéíóú ñ."
    $d.SaveAs2("$Salida\prueba.docx", 16); $d.Close()
} finally { Cerrar $w }

# Excel
$x = New-Object -ComObject Excel.Application; $x.Visible = $false; $x.DisplayAlerts = $false
try {
    $l = $x.Workbooks.Add(); $h = $l.Worksheets.Item(1)
    1..10 | % { $h.Cells.Item($_, 1).Value2 = "Fila $_"; $h.Cells.Item($_, 2).Value2 = $_ * 7 }
    $l.SaveAs("$Salida\prueba.xlsx", 51); $l.Close($false)
} finally { Cerrar $x }

# PowerPoint
$p = New-Object -ComObject PowerPoint.Application
try {
    $pr = $p.Presentations.Add($false)
    foreach ($i in 1..3) {
        $s = $pr.Slides.Add($i, 12)   # ppLayoutBlank
        $t = $s.Shapes.AddTextbox(1, 60, 60, 600, 80); $t.TextFrame.TextRange.Text = "Diapositiva $i de prueba"
    }
    $pr.SaveAs("$Salida\prueba.pptx"); $pr.Close()
} finally { Cerrar $p }

foreach ($e in 'docx', 'xlsx', 'pptx') {
    $pdf = "$Salida\prueba-$e.pdf"
    Remove-Item $pdf -EA 0
    & $Pliego create "$Salida\prueba.$e" -o $pdf
    $res = if ((Test-Path $pdf) -and $LASTEXITCODE -eq 0) { 'OK' } else { $ok = $false; 'FALLO' }
    "{0,-5} {1} ({2} bytes)" -f $e, $res, $(if (Test-Path $pdf) { (Get-Item $pdf).Length } else { 0 })
    if ($res -eq 'OK') { & $Pliego text $pdf | Select-Object -First 3 }
}
Get-Process WINWORD, EXCEL, POWERPNT -EA 0 | ? StartTime -gt (Get-Date).AddMinutes(-10) | Stop-Process -Force
if (-not $ok) { exit 1 }
