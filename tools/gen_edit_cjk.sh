#!/usr/bin/env bash
# Genera los 3 PDFs CJK del corpus de edición (japonés, chino, coreano) con `pliego edit insert-text`.
# Necesita el binario `pliego` compilado y una fuente CJK (por defecto, la que encuentre el sistema;
# o pásala con FONT_ZH/FONT_KO/FONT_JA=/ruta/fuente.ttc). Los PDFs resultantes llevan solo el subconjunto de glifos usado.
set -euo pipefail
cd "$(dirname "$0")/.."
PLIEGO=${PLIEGO:-target/debug/pliego}
DEST=tests/corpus/edit/corpus
BASE=$(mktemp --suffix=.pdf)
python3 tools/gen_edit_corpus30.py --blank "$BASE"
gen() { # nombre texto [fuente]
  local font=()
  [ -n "${3:-}" ] && font=(--font-file "$3")
  "$PLIEGO" edit insert-text "$BASE" --page 1 --rect 72,72,500,200 --size 16 "${font[@]}" --text "$2" -o "$DEST/$1.pdf" >/dev/null
}
gen cjk-japones "日本語の文章を 編集しても 周りの文字は まったく 変わりません。 これは 検証用の 例文です。" "${FONT_JA:-}"
gen cjk-chino "编辑一个词语之后 页面上的 其余内容 应当保持 完全不变。 这是 一段 用于 测试的 示例文字。" "${FONT_ZH:-}"
gen cjk-coreano "한 단어를 편집해도 나머지 글자는 전혀 바뀌지 않습니다. 이것은 검증용 예문입니다." "${FONT_KO:-}"
rm -f "$BASE"
ls -l "$DEST"/cjk-*.pdf
