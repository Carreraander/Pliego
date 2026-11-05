#!/usr/bin/env bash
# Fuzzing del motor (lopdf y PDFium; define PLIEGO_PDFIUM_DLL) con cargo-fuzz (requiere nightly: rustup toolchain install nightly; cargo install cargo-fuzz).
# Uso: tools/fuzz.sh [segundos por objetivo]   (por defecto 600; 3600 para la pasada de una hora)
set -euo pipefail
secs="${1:-600}"
cd "$(dirname "$0")/../crates/pliego-engine/fuzz"
for target in open render_page0 extract_text outline; do
  echo "== $target ($secs s)"
  cargo +nightly fuzz run "$target" -- -max_total_time="$secs" -rss_limit_mb=2048 -timeout=30
done
