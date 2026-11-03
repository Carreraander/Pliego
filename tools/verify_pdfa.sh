#!/usr/bin/env bash
# Convierte el corpus de prueba a PDF/A (1b, 2b, 2u, 3b) y lo valida con la validación propia y con veraPDF.
# Requiere: `pliego` compilado (cargo build -p pliego-cli) y veraPDF (Java) accesible por PATH o PLIEGO_VERAPDF.
# Uso: tools/verify_pdfa.sh [carpeta-de-salida]
set -u
root="$(cd "$(dirname "$0")/.." && pwd)"
out="${1:-$root/target/pdfa-verify}"
bin="${PLIEGO_BIN:-$root/target/debug/pliego}"
mkdir -p "$out"
fail=0; total=0; skipped=0
# Entradas: PDFs pequeños y sin cifrar; se excluyen los hostiles/de cuelgue y los ya firmados.
inputs=(basic/hola.pdf compare/base.pdf compare/imagen_a.pdf compare/parrafo.pdf organize/a.pdf organize/form1.pdf \
        organize/links.pdf protect/sucio.pdf redact/datos.pdf a11y/bueno.pdf a11y/malo.pdf)
for rel in "${inputs[@]}"; do
  for level in 1b 2b 2u 3b; do
    name="$(echo "$rel" | tr '/' '_' | sed 's/\.pdf$//')-$level.pdf"
    total=$((total+1))
    if ! "$bin" pdfa convert "$root/tests/corpus/$rel" -o "$out/$name" --level "$level" >"$out/$name.log" 2>&1; then
      echo "NO CONVERTIBLE  $rel [$level]: $(tail -1 "$out/$name.log")"; skipped=$((skipped+1)); continue
    fi
    if "$bin" pdfa validate "$out/$name" --level "$level" --verapdf >"$out/$name.vera.log" 2>&1; then
      echo "OK              $rel [$level]"
    else
      echo "FALLA           $rel [$level]"; sed 's/^/    /' "$out/$name.vera.log" | head -12; fail=$((fail+1))
    fi
  done
done
echo "Total: $total, fallan veraPDF: $fail, no convertibles: $skipped"
[ "$fail" -eq 0 ]
