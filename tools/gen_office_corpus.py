"""Genera tests/corpus/office/*.pdf: 20 PDFs sintéticos (cartas, facturas, artículos a dos columnas y tablas)
para probar la exportación a Word, Excel y PowerPoint (W14). Todo el contenido es ficticio.
Sin dependencias. Uso: python tools/gen_office_corpus.py"""
import pathlib
import random
import zlib

W, H = 595.0, 842.0
FONTS = {"F1": "Helvetica", "F2": "Helvetica-Bold", "F3": "Times-Roman", "F4": "Times-Bold",
         "F5": "Times-Italic", "F6": "Courier"}
PALABRAS = ("el la de que y en un una para con por los las del se su al lo como más pero sus le ya o este sí "
            "porque esta entre cuando muy sin sobre también me hasta hay donde quien desde todo nos durante "
            "todos uno les ni contra otros ese eso ante ellos e esto mí antes algunos qué unos yo otro otras "
            "otra él tanto esa estos mucho quienes nada muchos cual poco ella estar estas algunas algo nosotros "
            "proyecto documento servicio cliente acuerdo trabajo resultado análisis método sistema datos "
            "informe propuesta calidad plazo entrega revisión modelo prueba mejora proceso equipo gestión").split()


def ancho(texto, size, font="F1"):
    """Anchura aproximada en puntos (suficiente para alinear y partir líneas)."""
    base = {"F1": 0.52, "F2": 0.56, "F3": 0.45, "F4": 0.47, "F5": 0.43, "F6": 0.6}[font]
    w = 0.0
    for c in texto:
        if c.isdigit():
            w += 0.556 if font in ("F1", "F2") else 0.5
        elif c in ".,:;' ":
            w += 0.28 if font != "F6" else 0.6
        else:
            w += base
    return w * size


def esc(s):
    return s.replace("\\", "\\\\").replace("(", "\\(").replace(")", "\\)")


def latin(s):
    return s.encode("cp1252", "replace").decode("latin-1")


class Pagina:
    def __init__(self):
        self.ops = []
        self.imgs = []

    def texto(self, x, y, s, font="F1", size=11, color=(0, 0, 0)):
        """y = borde superior de la línea (coordenadas desde arriba)."""
        base = H - (y + size * 0.85)
        self.ops.append(f"BT /{font} {size} Tf {color[0]} {color[1]} {color[2]} rg {x:.2f} {base:.2f} Td "
                        f"({esc(latin(s))}) Tj ET")

    def derecha(self, x1, y, s, font="F1", size=11):
        self.texto(x1 - ancho(s, size, font), y, s, font, size)

    def linea(self, x0, y0, x1, y1, grosor=0.5):
        self.ops.append(f"{grosor} w {x0:.2f} {H - y0:.2f} m {x1:.2f} {H - y1:.2f} l S")

    def rect(self, x, y, w, h, relleno=None, grosor=0.5):
        if relleno:
            self.ops.append(f"{relleno[0]} {relleno[1]} {relleno[2]} rg {x:.2f} {H - y - h:.2f} {w:.2f} {h:.2f} re f")
        else:
            self.ops.append(f"{grosor} w {x:.2f} {H - y - h:.2f} {w:.2f} {h:.2f} re S")

    def imagen(self, x, y, w, h, semilla=0):
        n = len(self.imgs) + 1
        self.imgs.append((f"Im{n}", semilla))
        self.ops.append(f"q {w:.2f} 0 0 {h:.2f} {x:.2f} {H - y - h:.2f} cm /Im{n} Do Q")


def parrafo(texto, ancho_max, size, font="F1"):
    lineas, actual = [], ""
    for p in texto.split():
        prueba = (actual + " " + p).strip()
        if ancho(prueba, size, font) <= ancho_max or not actual:
            actual = prueba
        else:
            lineas.append(actual)
            actual = p
    if actual:
        lineas.append(actual)
    return lineas


def frase(rng, n):
    s = " ".join(rng.choice(PALABRAS) for _ in range(n))
    return s[0].upper() + s[1:] + "."


def texto_largo(rng, frases):
    return " ".join(frase(rng, rng.randint(8, 16)) for _ in range(frases))


def escribir(ruta, paginas):
    objs = []

    def add(s):
        objs.append(s)
        return len(objs)

    fids = {k: add(f"<< /Type /Font /Subtype /Type1 /BaseFont /{v} /Encoding /WinAnsiEncoding >>")
            for k, v in FONTS.items()}
    page_data = []
    for pg in paginas:
        stream = "\n".join(pg.ops).encode("latin-1")
        cid = add(("<< /Length %d >>\nstream\n" % len(stream)) + stream.decode("latin-1") + "\nendstream")
        xo = {}
        for name, seed in pg.imgs:
            iw, ih = 64, 48
            raw = bytearray()
            for yy in range(ih):
                for xx in range(iw):
                    raw += bytes([(xx * 4 + seed * 40) % 256, (yy * 5 + seed * 20) % 256, (xx + yy + seed * 70) % 256])
            comp = zlib.compress(bytes(raw))
            iid = add(f"<< /Type /XObject /Subtype /Image /Width {iw} /Height {ih} /ColorSpace /DeviceRGB "
                      f"/BitsPerComponent 8 /Filter /FlateDecode /Length {len(comp)} >>\nstream\n"
                      + comp.decode("latin-1") + "\nendstream")
            xo[name] = iid
        page_data.append((cid, xo))
    pages_id = len(objs) + len(page_data) + 1
    pids = []
    fonts = " ".join(f"/{k} {v} 0 R" for k, v in fids.items())
    for cid, xo in page_data:
        xs = " ".join(f"/{k} {v} 0 R" for k, v in xo.items())
        pids.append(add(f"<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 {W} {H}] /Resources << /Font << {fonts} >> "
                        f"/XObject << {xs} >> >> /Contents {cid} 0 R >>"))
    kids = " ".join(f"{p} 0 R" for p in pids)
    assert add(f"<< /Type /Pages /Kids [{kids}] /Count {len(pids)} >>") == pages_id
    catalog = add(f"<< /Type /Catalog /Pages {pages_id} 0 R >>")
    out = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
    offsets = []
    for i, body in enumerate(objs, 1):
        offsets.append(len(out))
        out += f"{i} 0 obj\n{body}\nendobj\n".encode("latin-1")
    xref = len(out)
    out += f"xref\n0 {len(objs) + 1}\n0000000000 65535 f \n".encode()
    for off in offsets:
        out += f"{off:010d} 00000 n \n".encode()
    out += f"trailer\n<< /Size {len(objs) + 1} /Root {catalog} 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode()
    ruta.write_bytes(bytes(out))


EMPRESAS = ["Talleres Norte S.L.", "Consultoría Aurora", "Editorial Cierzo", "Agroservicios Vega", "Estudio Lumbre"]
CLIENTES = ["Marta Ibáñez Ruiz", "Construcciones Pardo S.A.", "Asociación Los Olmos", "Julián Soto Vidal", "Clínica Mirador"]
CALLES = ["Calle Mayor 12", "Avenida del Sol 45", "Plaza Nueva 3", "Calle del Río 8", "Paseo de los Álamos 21"]
CIUDADES = ["28013 Madrid", "46002 Valencia", "41004 Sevilla", "50001 Zaragoza", "48005 Bilbao"]


def carta(i):
    rng = random.Random(100 + i)
    p = Pagina()
    fam, neg = ("F1", "F2") if i % 2 == 0 else ("F3", "F4")
    size = 11 if fam == "F1" else 12
    x0, x1 = 72, W - 72
    if i == 3:
        p.imagen(x0, 56, 64, 48, 2)
    p.texto(x0 if i != 3 else x0 + 76, 60, EMPRESAS[i - 1], neg, 16)
    for k, l in enumerate([CALLES[i - 1], CIUDADES[i - 1], "Tel. 600 000 00%d" % i]):
        p.derecha(x1, 60 + k * 14, l, fam, 10)
    y = 150
    p.texto(x0, y, f"En {CIUDADES[i - 1].split(' ', 1)[1]}, a {10 + i} de marzo de 2026", fam, size)
    y += 40
    for l in [CLIENTES[i - 1], CALLES[(i + 1) % 5], CIUDADES[(i + 2) % 5]]:
        p.texto(x0, y, l, fam, size)
        y += size * 1.3
    y += 24
    p.texto(x0, y, f"Asunto: propuesta de {rng.choice(PALABRAS)} {rng.choice(PALABRAS)}", neg, size)
    y += 30
    p.texto(x0, y, "Estimado cliente:", fam, size)
    y += 28
    for pi in range(3 if i != 4 else 2):
        for l in parrafo(texto_largo(rng, 4), x1 - x0, size, fam):
            p.texto(x0, y, l, fam, size)
            y += size * 1.35
        y += 10
        if i == 5 and pi == 0:  # lista con viñetas
            for it in range(3):
                p.texto(x0 + 10, y, "•", fam, size)
                for j, l in enumerate(parrafo(frase(rng, 14), x1 - x0 - 40, size, fam)):
                    p.texto(x0 + 30, y, l, fam, size)
                    y += size * 1.35
            y += 10
    p.texto(x0, y + 10, "Atentamente,", fam, size)
    p.texto(x0, y + 60, "Equipo de " + EMPRESAS[i - 1], neg, size)
    return [p]


def pie_pagina(p, fam, n, total, etiqueta):
    p.linea(56, 790, W - 56, 790)
    p.texto(56, 796, etiqueta, fam, 9)
    p.derecha(W - 56, 796, f"Página {n} de {total}", fam, 9)


def factura(i):
    rng = random.Random(200 + i)
    filas = [6, 9, 5, 12, 30][i - 1]
    paginas = []
    items = []
    for k in range(filas):
        cant = rng.randint(1, 20)
        precio = round(rng.uniform(3, 400), 2)
        iva = 21 if k % 4 else 10
        items.append((" ".join(rng.choice(PALABRAS) for _ in range(rng.randint(2, 4))).capitalize(),
                      cant, precio, iva, round(cant * precio, 2)))

    def euros(v):
        return f"{v:,.2f}".replace(",", "X").replace(".", ",").replace("X", ".")

    cols = [56, 300, 350, 430, 480]  # x0 de Descripción; x1 de Cant., Precio, IVA, Importe (derecha)
    per_page = 22
    chunks = [items[a:a + per_page] for a in range(0, len(items), per_page)]
    for n, chunk in enumerate(chunks, 1):
        p = Pagina()
        pie_pagina(p, "F1", n, len(chunks), f"Factura FA-2026-{i:04d}")
        y = 60
        if n == 1:
            p.texto(56, y, EMPRESAS[i - 1], "F2", 16)
            p.texto(56, y + 22, CALLES[i - 1] + ", " + CIUDADES[i - 1], "F1", 9)
            p.derecha(W - 56, y, "FACTURA", "F2", 20)
            p.derecha(W - 56, y + 26, f"N.º FA-2026-{i:04d}", "F1", 10)
            p.derecha(W - 56, y + 40, f"Fecha: {i + 3}/04/2026", "F1", 10)
            p.texto(56, 130, "Cliente", "F2", 10)
            p.texto(56, 144, CLIENTES[i - 1], "F1", 10)
            p.texto(56, 156, CALLES[(i + 2) % 5], "F1", 10)
            y = 200
        else:
            y = 90
        p.rect(50, y - 4, W - 100, 18, relleno=(0.9, 0.9, 0.9))
        p.texto(cols[0], y, "Descripción", "F2", 10)
        for x, t in zip(cols[1:], ["Cant.", "Precio", "IVA %", "Importe"]):
            p.derecha(x + 40 if t != "Cant." else x, y, t, "F2", 10)
        y += 22
        for d, c, pr, iva, imp in chunk:
            p.texto(cols[0], y, d, "F1", 10)
            p.derecha(cols[1], y, str(c), "F1", 10)
            p.derecha(cols[2] + 40, y, euros(pr), "F1", 10)
            p.derecha(cols[3] + 40, y, str(iva), "F1", 10)
            p.derecha(cols[4] + 40, y, euros(imp), "F1", 10)
            p.linea(50, y + 14, W - 50, y + 14, 0.25)
            y += 20
        if n == len(chunks):
            base = sum(t[4] for t in items)
            ivat = sum(t[4] * t[3] / 100 for t in items)
            y += 16
            for et, v, fn in [("Base imponible", base, "F1"), ("IVA", ivat, "F1"), ("TOTAL", base + ivat, "F2")]:
                p.texto(380, y, et, fn, 10)
                p.derecha(520, y, euros(round(v, 2)) + " €", fn, 10)
                y += 16
        paginas.append(p)
    return paginas


def articulo(i):
    rng = random.Random(300 + i)
    npag = [2, 2, 3, 2, 2][i - 1]
    paginas = []
    revista = f"Revista de {rng.choice(PALABRAS)} aplicada — 2026"
    gut = 18
    colw = (W - 112 - gut) / 2
    for n in range(npag):
        p = Pagina()
        p.derecha(W - 56, 30, revista, "F5", 9)
        y = 60
        if n == 0:
            p.texto(56, y, f"Un estudio sobre {rng.choice(PALABRAS)} y {rng.choice(PALABRAS)}", "F4", 20)
            p.texto(56, y + 30, "A. Navarro, B. Campos y C. Esteban", "F3", 11)
            y += 60
            for l in parrafo("Resumen. " + texto_largo(rng, 3), W - 112, 10, "F5"):
                p.texto(56, y, l, "F5", 10)
                y += 13
            y += 14
        top = y
        for c in range(2):
            x = 56 + c * (colw + gut)
            y = top
            if i == 4 and n == 0 and c == 1:
                p.imagen(x, y, 120, 90, 1)
                p.texto(x, y + 94, "Figura 1. Esquema del método.", "F5", 8)
                y += 116
            for s in range(3):
                p.texto(x, y, f"{s + 1 + c * 3 + n * 6}. {rng.choice(PALABRAS).capitalize()} {rng.choice(PALABRAS)}", "F4", 11)
                y += 18
                for l in parrafo(texto_largo(rng, 5), colw, 10, "F3"):
                    p.texto(x, y, l, "F3", 10)
                    y += 12.5
                    if y > 740:
                        break
                y += 8
                if i == 5 and s == 1:
                    for it in range(3):
                        p.texto(x + 6, y, f"{it + 1}.", "F3", 10)
                        for l in parrafo(frase(rng, 10), colw - 22, 10, "F3"):
                            p.texto(x + 22, y, l, "F3", 10)
                            y += 12.5
        p.texto(W / 2 - 5, 800, str(n + 1), "F3", 10)
        paginas.append(p)
    return paginas


def tabla(i):
    rng = random.Random(400 + i)
    ncols = [4, 5, 6, 7, 8][i - 1]
    nfil = [8, 10, 12, 14, 9][i - 1]
    p = Pagina()
    p.texto(56, 56, f"Informe trimestral {i}: resumen por zona", "F2", 16)
    p.texto(56, 80, texto_largo(rng, 1), "F1", 10)
    ancho_util = W - 112
    cw = ancho_util / ncols
    y = 120
    cab = ["Zona", "Responsable", "Enero", "Febrero", "Marzo", "Abril", "Mayo", "Total"][:ncols]
    if i == 4:
        p.rect(56, y - 3, ancho_util, 17, relleno=(0.85, 0.9, 1.0))
        p.texto(56 + 4, y, "Ventas por zona (euros)", "F2", 10)  # fila de título que abarca todas las columnas
        y += 20
    if i != 3:
        p.rect(56, y - 3, ancho_util, 17, relleno=(0.9, 0.9, 0.9))
    for c, t in enumerate(cab):
        if c < 2:
            p.texto(56 + c * cw + 4, y, t, "F2" if i != 3 else "F1", 10)
        else:
            p.derecha(56 + (c + 1) * cw - 4, y, t, "F2" if i != 3 else "F1", 10)
    y += 20
    for r in range(nfil):
        for c in range(ncols):
            if c == 0:
                p.texto(56 + 4, y, rng.choice(["Norte", "Sur", "Este", "Oeste", "Centro"]) + f" {r + 1}", "F1", 10)
            elif c == 1:
                p.texto(56 + cw + 4, y, rng.choice(["Ana", "Luis", "Marta", "Pedro", "Irene"]) + " " + rng.choice(["Gil", "Mora", "Ruiz"]), "F1", 10)
            else:
                v = f"{rng.randint(100, 99999):,}".replace(",", ".")
                p.derecha(56 + (c + 1) * cw - 4, y, v, "F1", 10)
        p.linea(56, y + 14, 56 + ancho_util, y + 14, 0.25)
        y += 18
    p.texto(56, y + 20, "Fuente: datos internos ficticios.", "F5", 9)
    return [p]


def main():
    out = pathlib.Path(__file__).resolve().parent.parent / "tests" / "corpus" / "office"
    out.mkdir(parents=True, exist_ok=True)
    for nombre, fn in [("carta", carta), ("factura", factura), ("articulo", articulo), ("tabla", tabla)]:
        for i in range(1, 6):
            escribir(out / f"{nombre}-{i}.pdf", fn(i))
    # PDF escaneado: una página con solo una imagen a página completa
    p = Pagina()
    p.imagen(0, 0, W, H, 3)
    escribir(out / "escaneado.pdf", [p])
    print(out)


if __name__ == "__main__":
    main()
