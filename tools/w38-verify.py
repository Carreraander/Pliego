#!/usr/bin/env python3
"""W38: comprobación independiente (pymupdf) de un PDF redactado: lo pedido ya no está y lo demás se conserva.

Uso: python tools/w38-verify.py salida.pdf --gone TEXTO [--gone TEXTO] --keep TEXTO [--keep TEXTO] [--region PAG,X0,Y0,X1,Y1]
`--region` (página desde 0, puntos, origen arriba a la izquierda, ESPACIO DE PÁGINA girada) debe quedar de un solo color a 150 ppp.
Salida: una línea por comprobación; código 1 si algo falla.
"""
import argparse
import sys

import fitz

ap = argparse.ArgumentParser()
ap.add_argument("pdf")
ap.add_argument("--gone", action="append", default=[])
ap.add_argument("--keep", action="append", default=[])
ap.add_argument("--region", action="append", default=[])
ap.add_argument("--varied", action="append", default=[], help="zona que NO debe ser uniforme (colateral)")
ap.add_argument("--no-field", action="append", default=[], help="valor que no debe quedar en ningún campo de formulario")
a = ap.parse_args()
doc = fitz.open(a.pdf)
text = "\n".join(p.get_text() for p in doc)
flat = "".join(text.split())
bad = 0
for g in a.gone:
    ok = "".join(g.split()) not in flat
    bad += not ok
    print(f"{'OK ' if ok else 'FALLA'} desaparecido: {g}")
for k in a.keep:
    ok = "".join(k.split()) in flat
    bad += not ok
    print(f"{'OK ' if ok else 'FALLA'} conservado:   {k}")
for r in a.region:
    pg, x0, y0, x1, y1 = (float(v) for v in r.split(","))
    # Se recorta 2,5 pt por cada lado: el píxel del borde sale mezclado por el suavizado de MuPDF y no es contenido.
    pix = doc[int(pg)].get_pixmap(matrix=fitz.Matrix(150 / 72, 150 / 72), clip=fitz.Rect(x0 + 2.5, y0 + 2.5, x1 - 2.5, y1 - 2.5), alpha=False)
    px = pix.samples
    n = pix.n
    first = px[:n]
    uniform = all(px[i:i + n] == first for i in range(0, len(px), n))
    bad += not uniform
    print(f"{'OK ' if uniform else 'FALLA'} zona uniforme {r}: {'sí' if uniform else 'no'} (color {tuple(first)})")
for r in a.varied:
    pg, x0, y0, x1, y1 = (float(v) for v in r.split(","))
    pix = doc[int(pg)].get_pixmap(matrix=fitz.Matrix(150 / 72, 150 / 72), clip=fitz.Rect(x0, y0, x1, y1), alpha=False)
    px, n = pix.samples, pix.n
    varied = any(px[i:i + n] != px[:n] for i in range(0, len(px), n))
    bad += not varied
    print(f"{'OK ' if varied else 'FALLA'} zona conservada {r}: {'con contenido' if varied else 'uniforme (¿borrada de más?)'}")
values = [str(w.field_value) for p in doc for w in (p.widgets() or [])]
for v in a.no_field:
    ok = not any(v in x for x in values)
    bad += not ok
    print(f"{'OK ' if ok else 'FALLA'} campo sin valor: {v}")
sys.exit(1 if bad else 0)
