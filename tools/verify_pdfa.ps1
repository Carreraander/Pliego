# Convierte el corpus de prueba a PDF/A (1b, 2b, 2u, 3b) y lo valida con la validación propia y con veraPDF.
# Requiere: `pliego` compilado (cargo build -p pliego-cli) y veraPDF (Java) en PATH o en PLIEGO_VERAPDF.
# Uso: powershell -File tools\verify_pdfa.ps1 [-Out carpeta]
param([string]$Out = "")
$root = Split-Path -Parent $PSScriptRoot
if (-not $Out) { $Out = Join-Path $root "target\pdfa-verify" }
$bin = if ($env:PLIEGO_BIN) { $env:PLIEGO_BIN } else { Join-Path $root "target\debug\pliego.exe" }
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$inputs = "basic/hola.pdf","compare/base.pdf","compare/imagen_a.pdf","compare/parrafo.pdf","organize/a.pdf","organize/form1.pdf",
          "organize/links.pdf","protect/sucio.pdf","redact/datos.pdf","a11y/bueno.pdf","a11y/malo.pdf"
$fail = 0; $total = 0; $skipped = 0
foreach ($rel in $inputs) {
  foreach ($level in "1b","2b","2u","3b") {
    $name = ($rel -replace "/","_" -replace "\.pdf$","") + "-$level.pdf"
    $total++
    $src = Join-Path $root ("tests\corpus\" + ($rel -replace "/","\"))
    & $bin pdfa convert $src -o (Join-Path $Out $name) --level $level *> (Join-Path $Out "$name.log")
    if ($LASTEXITCODE -ne 0) {
      "NO CONVERTIBLE  $rel [$level]: " + (Get-Content (Join-Path $Out "$name.log") | Select-Object -Last 1)
      $skipped++; continue
    }
    & $bin pdfa validate (Join-Path $Out $name) --level $level --verapdf *> (Join-Path $Out "$name.vera.log")
    if ($LASTEXITCODE -eq 0) { "OK              $rel [$level]" }
    else {
      "FALLA           $rel [$level]"
      Get-Content (Join-Path $Out "$name.vera.log") | Select-Object -First 12 | ForEach-Object { "    $_" }
      $fail++
    }
  }
}
"Total: $total, fallan veraPDF: $fail, no convertibles: $skipped"
exit ($(if ($fail -eq 0) { 0 } else { 1 }))
