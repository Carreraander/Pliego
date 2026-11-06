#!/usr/bin/env python3
"""W38: PDFs de entrada para redactar con el motor propio en acr12 (texto, foto, vectores, página girada, escaneados, formulario).

Uso: python tools/w38-gen-inputs.py <carpeta de salida> [<carpeta con los documentos de tools/w37-gen-docs.py>]
Parte de tests/corpus/redact/datos.pdf (datos ficticios) y de los PDF de W37 si se indica la carpeta. Necesita pymupdf.
"""
import io
import sys
from pathlib import Path

import fitz
from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parent.parent
out = Path(sys.argv[1])
out.mkdir(parents=True, exist_ok=True)
docs = Path(sys.argv[2]) if len(sys.argv) > 2 else None
datos = REPO / "tests/corpus/redact/datos.pdf"


def save(doc, name):
    p = out / name
    doc.save(p, deflate=True)
    print(p)


# texto + imagen pequeña + vector (original) y las mismas con página girada.
for rot in (0, 90, 180, 270):
    d = fitz.open(datos)
    if rot:
        for pg in d:
            pg.set_rotation(rot)
    save(d, f"datos-giro{rot}.pdf")

# foto JPEG de verdad con texto vectorial debajo y encima.
photo = Image.new("RGB", (900, 1200))
dr = ImageDraw.Draw(photo)
for y in range(1200):
    dr.line((0, y, 900, y), fill=(y % 255, (y * 3) % 255, 255 - y % 255))
for i in range(40):
    dr.ellipse((i * 20, i * 25, i * 20 + 200, i * 25 + 200), outline=(255, 255, 255), width=4)
b = io.BytesIO()
photo.save(b, "JPEG", quality=88)
d = fitz.open()
p = d.new_page(width=595, height=842)
p.insert_image(fitz.Rect(50, 100, 500, 700), stream=b.getvalue())
p.insert_text((50, 60), "Foto con DNI 12345678Z en el pie", fontsize=14)
p.insert_text((50, 740), "Pie: IBAN ES91 2100 0418 4502 0005 1332", fontsize=12)
save(d, "foto-jpeg.pdf")

# vectores: curvas con transparencia bajo la zona a redactar.
d = fitz.open()
p = d.new_page(width=595, height=842)
sh = p.new_shape()
for i in range(60):
    sh.draw_rect(fitz.Rect(40 + i * 7, 100 + i * 9, 140 + i * 7, 200 + i * 9))
    sh.finish(color=(0, 0, i / 60), fill=(i / 60, 0.3, 1 - i / 60), fill_opacity=0.5, width=1)
sh.commit()
p.insert_text((50, 60), "Plano. Contacto ana.lopez@ejemplo.es", fontsize=14)
save(d, "vectores.pdf")

if docs:
    for name, pages in (("escaneado-gris-40", 2), ("escaneado-1bit-40", 2), ("formulario-20", 2), ("fotos-30", 2)):
        src = docs / f"{name}.pdf"
        if src.exists():
            d = fitz.open(src)
            d.select(list(range(pages)))
            save(d, f"{name.rsplit('-', 1)[0]}-2pag.pdf")
