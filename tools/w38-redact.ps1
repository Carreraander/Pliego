<#
.SYNOPSIS
  W38: redacta con el motor propio (PLIEGO_REDACT_ENGINE=native) una serie de PDF, pasa el oráculo de W34 con candidato pdfium
  y una comprobación independiente (pymupdf), y deja los PDF redactados en la carpeta de salida para abrirlos en Reader.
.DESCRIPTION
  Requiere `cargo build --release -p pliego-cli -p pliego-qa` y pdfium.dll. Por turno:
  .\tools\acr12-turno.ps1 -Flujo gestion -- powershell -File .\tools\w38-redact.ps1 -Dll D:\dev-cache\pdfium\pdfium.dll -Docs D:\dev-cache\w37\docs
#>
[CmdletBinding()]
param([Parameter(Mandatory)][string]$Dll, [string]$Docs, [string]$Out = 'D:\dev-cache\w38-redaccion')
$ErrorActionPreference = 'Continue'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$rel = Join-Path ($(if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $repo 'target' })) 'release'
$cli = Join-Path $rel 'pliego.exe'; $oracle = Join-Path $rel 'redaction-oracle.exe'
foreach ($f in $cli, $oracle) { if (-not (Test-Path $f)) { throw "Falta $f (cargo build --release -p pliego-cli -p pliego-qa)" } }
$env:PLIEGO_PDFIUM_DLL = $Dll
$in = Join-Path $Out 'entradas'; $res = Join-Path $Out 'redactados'
New-Item -ItemType Directory -Force $in, $res | Out-Null
python (Join-Path $PSScriptRoot 'w38-gen-inputs.py') $in $(if ($Docs) { $Docs })

$datos = @('12345678Z', 'X1234567L', 'A58818501', 'ES91 2100 0418 4502 0005 1332', 'ana.lopez@ejemplo.es', '+34 612 345 678', '12/03/1985')
$pat = 'dni,nie,cif,iban,email,telefono,fecha'
$keepTxt = @('Ana Lopez', 'texto normal conservado', 'Segunda pagina')
# Zonas (espacio de página girada, puntos, origen arriba a la izquierda) de la imagen 4x4 y del rectángulo rojo de datos.pdf.
$geo = @{
  0   = @{ img = '72,442,172,542';  rect = '300,442,400,542' }
  90  = @{ img = '300,72,400,172';  rect = '300,300,400,400' }
  180 = @{ img = '423,300,523,400'; rect = '195,300,295,400' }
  270 = @{ img = '442,423,542,523'; rect = '442,195,542,295' }
}
$cases = @()
foreach ($r in 0, 90, 180, 270) {
  $g = $geo[$r]
  $cases += @{ n = "texto-giro$r"; i = "datos-giro$r.pdf"; a = @('--pattern', $pat); gone = $datos; keep = $keepTxt; reg = @(); var = @() }
  $cases += @{ n = "imagen-giro$r"; i = "datos-giro$r.pdf"; a = @('--area', "1:$($g.img)"); gone = @(); keep = $keepTxt + $datos; reg = @("0,$($g.img)"); var = @() }
  $cases += @{ n = "vector-giro$r"; i = "datos-giro$r.pdf"; a = @('--area', "1:$($g.rect)"); gone = @(); keep = $keepTxt + $datos; reg = @("0,$($g.rect)"); var = @() }
}
$cases += @{ n = 'foto-jpeg'; i = 'foto-jpeg.pdf'; a = @('--pattern', 'dni,iban', '--area', '1:100,200,300,400'); gone = @('12345678Z', 'ES91 2100 0418 4502 0005 1332'); keep = @('Foto con', 'Pie:'); reg = @('0,100,200,300,400'); var = @('0,320,450,480,650') }
$cases += @{ n = 'vectores'; i = 'vectores.pdf'; a = @('--pattern', 'email', '--area', '1:150,200,350,400'); gone = @('ana.lopez@ejemplo.es'); keep = @('Plano', 'Contacto'); reg = @('0,150,200,350,400'); var = @('0,400,600,590,780') }
if ($Docs) {
  $cases += @{ n = 'escaneado-gris'; i = 'escaneado-gris-2pag.pdf'; a = @('--area', '1:100,100,400,200', '--area', '2:100,300,400,420'); gone = @(); keep = @(); reg = @('0,100,100,400,200', '1,100,300,400,420'); var = @('0,100,500,400,600') }
  $cases += @{ n = 'escaneado-1bit'; i = 'escaneado-1bit-2pag.pdf'; a = @('--area', '1:100,100,400,200', '--area', '2:100,300,400,420'); gone = @(); keep = @(); reg = @('0,100,100,400,200', '1,100,300,400,420'); var = @('0,100,500,400,600') }
  $cases += @{ n = 'fotos-jpeg'; i = 'fotos-2pag.pdf'; a = @('--area', '1:100,100,300,300'); gone = @(); keep = @(); reg = @('0,100,100,300,300'); var = @('0,300,400,500,650') }
  $cases += @{ n = 'formulario'; i = 'formulario-2pag.pdf'; a = @('--area', '1:140,120,410,146'); gone = @('valor 0-2'); keep = @('Solicitud 1', 'Campo 3'); reg = @('0,140,120,410,146'); var = @(); nofield = @('valor 0-2') }
}
$rows = @()
foreach ($c in $cases) {
  $src = Join-Path $in $c.i; $dst = Join-Path $res "$($c.n).pdf"
  if (-not (Test-Path $src)) { $rows += [pscustomobject]@{ caso = $c.n; redaccion = 'sin entrada'; oraculo_pdfium = '-'; oraculo_mupdf = '-'; pymupdf = '-'; qpdf = '-'; salida = '' }; continue }
  $env:PLIEGO_REDACT_ENGINE = 'native'
  $log = & $cli redact $src @($c.a) --apply -o $dst 2>&1 | Out-String
  $okRedact = ($LASTEXITCODE -eq 0) -and (Test-Path $dst)
  Set-Content -Path (Join-Path $res "$($c.n).log") -Value $log -Encoding UTF8
  $oa = @('--original', $src); foreach ($g in $c.gone) { $oa += '--needle', $g }; foreach ($r in $c.reg) { $oa += '--region', $r }
  $op = $om = $py = $qp = '-'
  if ($okRedact) {
    if ($oa.Count -gt 2) {
      $o = & $oracle $dst --engine pdfium @oa 2>&1 | Out-String; $op = if ($LASTEXITCODE -eq 0) { 'OK' } else { "FALLA`n$o" }
    }
    $va = @(); foreach ($g in $c.gone) { $va += '--gone', $g }; foreach ($k in $c.keep) { $va += '--keep', $k }
    foreach ($r in $c.reg) { $va += '--region', $r }; foreach ($v in $c.var) { $va += '--varied', $v }; foreach ($f in $c.nofield) { $va += '--no-field', $f }
    $o = python (Join-Path $PSScriptRoot 'w38-verify.py') $dst @va 2>&1 | Out-String
    $py = if ($LASTEXITCODE -eq 0) { 'OK' } else { "FALLA`n$o" }
    $o = qpdf --check $dst 2>&1 | Out-String; $qp = if ($o -match 'No syntax or stream encoding errors') { 'OK' } else { "FALLA`n$o" }
  }
  $rows += [pscustomobject]@{ caso = $c.n; redaccion = $(if ($okRedact) { 'OK' } else { "FALLA`n$log" }); oraculo_pdfium = $op; oraculo_mupdf = $om; pymupdf = $py; qpdf = $qp; salida = $dst }
}
$rows | ConvertTo-Json -Depth 3 | Set-Content (Join-Path $Out 'resultado.json') -Encoding UTF8
$rows | Format-Table caso, redaccion, oraculo_pdfium, oraculo_mupdf, pymupdf, qpdf -AutoSize | Out-String -Width 200
Write-Host "Redactados en $res"
