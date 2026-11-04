"""Genera los PDFs para medir W04 (no se suben al repo): 50 PDFs de 20 páginas con marcadores en <salida>/in/.
Uso: python tools/gen_w04_bench.py [carpeta_salida]. Sin dependencias."""
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import gen_organize_corpus as g

out = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ".") / "in"
out.mkdir(parents=True, exist_ok=True)
for i in range(50):
    (out / f"doc{i:02d}.pdf").write_bytes(g.build(f"D{i:02d}", 20))
print(f"50 PDFs de 20 páginas en {out}")
