#!/usr/bin/env python3
"""W37: genera documentos «reales» (fotos, escaneados, formularios, vectores, 1.000 páginas mixtas) para medir motores.

Uso: python tools/w37-gen-docs.py <carpeta de salida> [nombre ...]  (sin nombres, genera todos)      (necesita pymupdf, pillow, reportlab; es determinista)
Salida: fotos-30.pdf, escaneado-gris-40.pdf, escaneado-1bit-40.pdf, formulario-20.pdf, vectores-10.pdf, mixto-1000.pdf.
No va en el repositorio como corpus (pesa decenas de MB): se regenera en D:\\dev-cache\\w37\\docs.
"""
import io
import random
import sys
from pathlib import Path

import fitz  # pymupdf
from PIL import Image, ImageDraw, ImageFilter, ImageFont

A4 = fitz.paper_rect("a4")
random.seed(37)
WORDS = "pliego visor editor documento página contrato factura informe cliente importe fecha firma anexo cláusula".split()


def font(size):
    for f in (r"C:\Windows\Fonts\arial.ttf", "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"):
        try:
            return ImageFont.truetype(f, size)
        except OSError:
            pass
    return ImageFont.load_default()


def photo(w, h, seed):
    rnd = random.Random(seed)
    base = Image.effect_noise((w // 8, h // 8), 64).convert("RGB").resize((w, h), Image.BICUBIC)
    d = ImageDraw.Draw(base)
    for _ in range(60):
        x, y = rnd.randrange(w), rnd.randrange(h)
        r = rnd.randrange(40, 400)
        d.ellipse((x - r, y - r, x + r, y + r), fill=tuple(rnd.randrange(256) for _ in range(3)))
    return base.filter(ImageFilter.GaussianBlur(2)).convert("RGB")


def text_page(w, h, dpi_scale, seed):
    rnd = random.Random(seed)
    im = Image.new("L", (w, h), 245)
    d = ImageDraw.Draw(im)
    f = font(int(34 * dpi_scale))
    y = int(150 * dpi_scale)
    while y < h - 150 * dpi_scale:
        d.text((int(120 * dpi_scale), y), " ".join(rnd.choice(WORDS) for _ in range(11)), fill=20, font=f)
        y += int(52 * dpi_scale)
    noise = Image.effect_noise((w, h), 18).convert("L")
    return Image.blend(im, noise, 0.08).rotate(rnd.uniform(-0.6, 0.6), fillcolor=240)


def jpeg(im, q=82):
    b = io.BytesIO()
    im.save(b, "JPEG", quality=q)
    return b.getvalue()


def fotos(path):
    doc = fitz.open()
    for i in range(30):
        p = doc.new_page(width=A4.width, height=A4.height)
        p.insert_image(fitz.Rect(30, 30, A4.width - 30, A4.height - 120), stream=jpeg(photo(1500, 2100, i), 85))
        p.insert_text((40, A4.height - 60), f"Foto {i + 1}: pie de imagen con texto vectorial", fontsize=12)
    doc.save(path, deflate=True)


def escaneado_gris(path):
    doc = fitz.open()
    for i in range(40):
        p = doc.new_page(width=A4.width, height=A4.height)
        p.insert_image(A4, stream=jpeg(text_page(2480, 3508, 1.0, i), 70))
    doc.save(path, deflate=True)


def escaneado_1bit(path):
    doc = fitz.open()
    for i in range(40):
        im = text_page(2480, 3508, 1.0, 1000 + i).point(lambda v: 255 if v > 150 else 0).convert("1")
        b = io.BytesIO()
        im.save(b, "PNG")
        p = doc.new_page(width=A4.width, height=A4.height)
        p.insert_image(A4, stream=b.getvalue())
    doc.save(path, deflate=True)


def formulario(path):
    from reportlab.lib.pagesizes import A4 as RA4
    from reportlab.pdfgen import canvas

    c = canvas.Canvas(str(path), pagesize=RA4)
    for pg in range(20):
        c.setFont("Helvetica-Bold", 14)
        c.drawString(50, 800, f"Solicitud {pg + 1}")
        c.setFont("Helvetica", 10)
        y = 760
        for i in range(12):
            c.drawString(50, y, f"Campo {i + 1}")
            c.acroForm.textfield(name=f"t{pg}_{i}", value=f"valor {pg}-{i}", x=150, y=y - 4, width=250, height=18, borderStyle="inset")
            y -= 28
        c.acroForm.checkbox(name=f"c{pg}", x=50, y=y - 4, size=14, checked=pg % 2 == 0)
        c.acroForm.choice(name=f"l{pg}", value="B", options=["A", "B", "C"], x=150, y=y - 4, width=120, height=18)
        c.acroForm.radio(name=f"r{pg}", value="si", selected=True, x=300, y=y - 4, size=14)
        c.acroForm.radio(name=f"r{pg}", value="no", selected=False, x=330, y=y - 4, size=14)
        c.showPage()
    c.save()


def vectores(path):
    rnd = random.Random(5)
    doc = fitz.open()
    for _ in range(10):
        p = doc.new_page(width=A4.width, height=A4.height)
        sh = p.new_shape()
        for _ in range(4000):
            pts = [fitz.Point(rnd.uniform(0, A4.width), rnd.uniform(0, A4.height)) for _ in range(4)]
            sh.draw_bezier(*pts)
            sh.finish(color=(rnd.random(), rnd.random(), rnd.random()), fill=(rnd.random(), rnd.random(), rnd.random()),
                      fill_opacity=rnd.uniform(0.1, 0.9), width=rnd.uniform(0.2, 2))
        sh.commit()
    doc.save(path, deflate=True)


def mixto(path):
    imgs = [jpeg(photo(600, 400, 900 + k), 80) for k in range(6)]
    doc = fitz.open()
    for i in range(1000):
        p = doc.new_page(width=A4.width, height=A4.height)
        p.insert_textbox(fitz.Rect(50, 50, 545, 450), "\n".join(
            f"Pág. {i + 1} línea {j + 1}: " + " ".join(random.choice(WORDS) for _ in range(9)) for j in range(28)), fontsize=10)
        p.insert_image(fitz.Rect(50, 470, 400, 700), stream=imgs[i % 6])
    doc.save(path, deflate=True)


if __name__ == "__main__":
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    for name, fn in (("fotos-30", fotos), ("escaneado-gris-40", escaneado_gris), ("escaneado-1bit-40", escaneado_1bit),
                     ("formulario-20", formulario), ("vectores-10", vectores), ("mixto-1000", mixto)):
        if len(sys.argv) > 2 and name not in sys.argv[2:]:
            continue
        f = out / f"{name}.pdf"
        fn(f)
        print(f"{f}  {f.stat().st_size / 1e6:.1f} MB", flush=True)
