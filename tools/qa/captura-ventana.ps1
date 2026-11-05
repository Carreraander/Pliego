<#
.SYNOPSIS
  Captura de pantalla de UNA ventana (por proceso y/o título) y volcado de su árbol de UI Automation a texto y JSON.

.DESCRIPTION
  Pensado para validar a mano la app sin control de pantalla remoto (computer use): se lanza la app, y esto deja
  `<nombre>.png`, `<nombre>.uia.txt` y (con -Json) `<nombre>.uia.json` en la carpeta de salida.
  - La imagen se toma con CopyFromScreen sobre el rectángulo de la ventana, es decir, sale lo que haya VISIBLE ahí: si otra
    ventana la tapa, saldrá la otra. Use -Traer para dar el foco a la ventana antes (UIA SetFocus; cambia el foco del usuario).
  - Se informa de los colores distintos de la imagen para detectar capturas en negro (pantalla bloqueada, escritorio seguro).
  - Los valores y nombres de los campos que UIA marca como contraseña/PIN (IsPassword) NO se vuelcan. Limitación: el nombre
    de cada control lo decide la aplicación; algunas (p. ej. WinForms antiguo con UseSystemPasswordChar) exponen el texto
    de un campo de contraseña como nombre sin marcarlo. Revise el volcado antes de compartirlo. La imagen sí muestra
    lo que haya en pantalla: no la comparta si salen datos privados.
  - En un cuadro de Windows como el de PIN de tarjeta (clase `Credential Dialog Xaml Host`) CopyFromScreen puede no mostrarlo;
    el volcado UIA sí lo lista.
  - Dentro del AMSI de algunos antivirus, este código lanzado «en línea» se bloquea; ejecútelo desde el fichero .ps1.

.PARAMETER Proceso
  Nombre del proceso sin .exe (p. ej. pliego, notepad). Admite comodines.
.PARAMETER Titulo
  Título de la ventana (comodines de -like, p. ej. '*Pliego*').
.PARAMETER Clase
  Clase de la ventana (comodines de -like), p. ej. 'Credential Dialog Xaml Host'.
.PARAMETER Salida
  Carpeta de salida (por defecto C:\dev-cache\qa\capturas).
.PARAMETER Nombre
  Nombre base de los ficheros (por defecto, proceso y hora).
.PARAMETER Profundidad
  Profundidad máxima del árbol UIA (por defecto 12). Tope de 6000 nodos.
.PARAMETER EsperaSeg
  Segundos que se espera a que aparezca la ventana (por defecto 10).
.PARAMETER Json
  Escribe también el árbol como JSON.
.PARAMETER Traer
  Restaura y da el foco a la ventana antes de capturar.
.PARAMETER SoloArbol
  No hace captura de imagen, solo el volcado UIA.
.PARAMETER Lista
  Solo lista las ventanas de primer nivel (proceso, PID, clase, título, rectángulo) y sale.

.EXAMPLE
  .\tools\qa\captura-ventana.ps1 -Proceso pliego -Titulo '*Pliego*' -Json -Traer
.EXAMPLE
  .\tools\qa\captura-ventana.ps1 -Lista

  Código de salida: 0 = hecho; 1 = no apareció la ventana; 2 = error al capturar.
#>
[CmdletBinding()]
param(
  [string]$Proceso,
  [string]$Titulo,
  [string]$Clase,
  [string]$Salida = 'C:\dev-cache\qa\capturas',
  [string]$Nombre,
  [int]$Profundidad = 12,
  [int]$EsperaSeg = 10,
  [switch]$Json,
  [switch]$Traer,
  [switch]$SoloArbol,
  [switch]$Lista
)
Set-StrictMode -Version 2
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

# UIA devuelve píxeles físicos; sin esto, con escala de pantalla distinta de 100 % CopyFromScreen usa píxeles lógicos y el recorte se descuadra.
Add-Type -TypeDefinition 'using System.Runtime.InteropServices; public static class PliegoDpi { [DllImport("user32.dll")] public static extern bool SetProcessDPIAware(); }'
[void][PliegoDpi]::SetProcessDPIAware()

$AE = [System.Windows.Automation.AutomationElement]
$root = $AE::RootElement
$tope = 6000
$nodos = 0

function Info-Ventana($w) {
  $c = $w.Current
  $r = $c.BoundingRectangle
  $nom = try { (Get-Process -Id $c.ProcessId -ErrorAction Stop).ProcessName } catch { '?' }
  [pscustomobject]@{
    Elemento = $w; Proceso = $nom; Pid = $c.ProcessId; Clase = $c.ClassName; Titulo = $c.Name
    X = if ([double]::IsInfinity($r.X)) { 0 } else { [int]$r.X }; Y = if ([double]::IsInfinity($r.Y)) { 0 } else { [int]$r.Y }
    An = if ([double]::IsInfinity($r.Width)) { 0 } else { [int]$r.Width }; Al = if ([double]::IsInfinity($r.Height)) { 0 } else { [int]$r.Height }
    Fuera = $c.IsOffscreen
  }
}

function Ventanas { $root.FindAll('Children', [System.Windows.Automation.Condition]::TrueCondition) | ForEach-Object { Info-Ventana $_ } }

if ($Lista) {
  Ventanas | Where-Object { $_.An -gt 0 -and $_.Al -gt 0 } |
    Select-Object Proceso, Pid, Clase, Titulo, X, Y, An, Al, Fuera | Format-Table -AutoSize -Wrap | Out-String -Width 250 | Write-Host
  exit 0
}
if (-not ($Proceso -or $Titulo -or $Clase)) { throw 'Indique -Proceso, -Titulo y/o -Clase (o -Lista para ver las ventanas).' }

# ---- Buscar la ventana (se espera a que aparezca) --------------------------------------------------------------------
$fin = (Get-Date).AddSeconds($EsperaSeg)
$ventana = $null
do {
  $cand = @(Ventanas | Where-Object {
      $_.An -gt 0 -and $_.Al -gt 0 -and
      (-not $Proceso -or $_.Proceso -like $Proceso) -and (-not $Titulo -or $_.Titulo -like $Titulo) -and (-not $Clase -or $_.Clase -like $Clase)
    } | Sort-Object { $_.An * $_.Al } -Descending)
  if ($cand.Count -gt 0) { $ventana = $cand[0] } else { Start-Sleep -Milliseconds 400 }
} while (-not $ventana -and (Get-Date) -lt $fin)
if (-not $ventana) { Write-Host "No apareció ninguna ventana (proceso='$Proceso' título='$Titulo' clase='$Clase')."; exit 1 }
if ($cand.Count -gt 1) { Write-Host "Aviso: $($cand.Count) ventanas coinciden; se usa la mayor ('$($ventana.Titulo)')." }

if ($Traer) {
  try {
    $wp = $ventana.Elemento.GetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern)
    if ($wp.Current.WindowVisualState -eq 'Minimized') { $wp.SetWindowVisualState('Normal') }
  } catch {}
  try { $ventana.Elemento.SetFocus() } catch { Write-Warning "no se pudo dar el foco: $($_.Exception.Message)" }
  Start-Sleep -Milliseconds 700
  $ventana = Info-Ventana $ventana.Elemento   # el rectángulo puede haber cambiado al restaurar
}

New-Item -ItemType Directory -Force $Salida | Out-Null
if (-not $Nombre) {
  $base = if ($Proceso) { $Proceso -replace '[^\w\-]', '' } else { 'ventana' }
  $Nombre = '{0}-{1}' -f $base, (Get-Date -Format 'yyyyMMdd-HHmmss')
}
$Nombre = $Nombre -replace '[\\/:*?"<>|]', '_'
Write-Host ("Ventana: [{0}] pid={1} clase='{2}' título='{3}' rect={4},{5} {6}x{7}" -f $ventana.Proceso, $ventana.Pid, $ventana.Clase, $ventana.Titulo, $ventana.X, $ventana.Y, $ventana.An, $ventana.Al)

# ---- Captura -----------------------------------------------------------------------------------------------------------
$codigo = 0
if (-not $SoloArbol) {
  try {
    $v = [System.Windows.Forms.SystemInformation]::VirtualScreen
    $x0 = [Math]::Max($ventana.X, $v.X); $y0 = [Math]::Max($ventana.Y, $v.Y)
    $x1 = [Math]::Min($ventana.X + $ventana.An, $v.X + $v.Width); $y1 = [Math]::Min($ventana.Y + $ventana.Al, $v.Y + $v.Height)
    if ($x1 -le $x0 -or $y1 -le $y0) { throw 'la ventana está fuera de la pantalla (¿minimizada?; pruebe -Traer)' }
    $bmp = New-Object System.Drawing.Bitmap ($x1 - $x0), ($y1 - $y0)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($x0, $y0, 0, 0, $bmp.Size)
    $g.Dispose()
    $png = Join-Path $Salida "$Nombre.png"
    $bmp.Save($png, [System.Drawing.Imaging.ImageFormat]::Png)
    $cols = New-Object 'System.Collections.Generic.HashSet[int]'
    $paso = 7
    for ($yy = 0; $yy -lt $bmp.Height; $yy += $paso) { for ($xx = 0; $xx -lt $bmp.Width; $xx += $paso) { [void]$cols.Add($bmp.GetPixel($xx, $yy).ToArgb()) } }
    $bmp.Dispose()
    Write-Host ("Captura: {0} ({1}x{2}, {3} colores distintos en la muestra){4}" -f $png, ($x1 - $x0), ($y1 - $y0), $cols.Count,
      $(if ($cols.Count -le 2) { '  AVISO: casi uniforme; puede ser negro/vacío o estar tapada' } else { '' }))
  } catch {
    Write-Warning "Captura fallida: $($_.Exception.Message)"
    $codigo = 2
  }
}

# ---- Árbol UIA -----------------------------------------------------------------------------------------------------------
$walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
function Nodo($el, [int]$nivel) {
  $script:nodos++
  $c = $el.Current
  $r = $c.BoundingRectangle
  $valor = $null
  if (-not $c.IsPassword) {
    try { $valor = $el.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value } catch {}
  }
  $n = [ordered]@{
    tipo = ($c.ControlType.ProgrammaticName -replace '^ControlType\.', ''); nombre = $(if ($c.IsPassword) { '' } else { $c.Name }); id = $c.AutomationId; clase = $c.ClassName
    rect = if ([double]::IsInfinity($r.X)) { $null } else { @([int]$r.X, [int]$r.Y, [int]$r.Width, [int]$r.Height) }
    habilitado = $c.IsEnabled; fuera = $c.IsOffscreen; clave = $c.IsPassword
  }
  if ($valor) { $n.valor = $valor }
  $hijos = @()
  if ($nivel -lt $Profundidad -and $script:nodos -lt $tope) {
    $h = $walker.GetFirstChild($el)
    while ($h -and $script:nodos -lt $tope) { $hijos += , (Nodo $h ($nivel + 1)); $h = $walker.GetNextSibling($h) }
  }
  $n.hijos = $hijos
  $n
}
function Texto($n, [int]$nivel, $sb) {
  $extra = @()
  if ($n.id) { $extra += "id='$($n.id)'" }
  if ($n.clase) { $extra += "clase=$($n.clase)" }
  if ($n.rect) { $extra += "rect=$($n.rect -join ',')" }
  if (-not $n.habilitado) { $extra += 'deshabilitado' }
  if ($n.fuera) { $extra += 'fuera-de-pantalla' }
  if ($n.clave) { $extra += 'CLAVE(valor omitido)' }
  if ($n.Contains('valor')) { $extra += "valor='$($n.valor)'" }
  [void]$sb.AppendLine(('{0}{1} ''{2}'' {3}' -f ('  ' * $nivel), $n.tipo, $n.nombre, ($extra -join ' ')))
  foreach ($h in $n.hijos) { Texto $h ($nivel + 1) $sb }
}
$arbol = Nodo $ventana.Elemento 0
$sb = New-Object System.Text.StringBuilder
Texto $arbol 0 $sb
$txt = Join-Path $Salida "$Nombre.uia.txt"
[IO.File]::WriteAllText($txt, $sb.ToString(), (New-Object System.Text.UTF8Encoding($true)))
Write-Host "Árbol UIA: $txt ($nodos nodos$(if ($nodos -ge $tope) { ', recortado al tope' }))"
if ($Json) {
  $js = Join-Path $Salida "$Nombre.uia.json"
  $doc = [ordered]@{ proceso = $ventana.Proceso; pid = $ventana.Pid; clase = $ventana.Clase; titulo = $ventana.Titulo; fecha = (Get-Date).ToString('o'); arbol = $arbol }
  [IO.File]::WriteAllText($js, ($doc | ConvertTo-Json -Depth 60), (New-Object System.Text.UTF8Encoding($true)))
  Write-Host "JSON: $js"
}
exit $codigo
