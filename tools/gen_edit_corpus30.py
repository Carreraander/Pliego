"""Genera tests/corpus/edit/corpus/*.pdf: 27 PDFs sintéticos para probar la edición de texto (los 3 de CJK
los crea `pliego edit insert-text`; ver tools/gen_edit_cjk.sh). Imitan lo que producen Word, LaTeX y los
escáneres con OCR: fuentes y tamaños distintos, TJ con ajustes, Tm por línea, páginas giradas, cajas con
desplazamiento, varios flujos de contenido, flujos comprimidos, dos columnas, imágenes y vectores.
Sin dependencias. Uso: python tools/gen_edit_corpus30.py"""
import pathlib
import sys
import zlib

LOREM = [
    "La edición de párrafos conserva el aspecto del resto de la página.",
    "Un segundo párrafo más largo sirve para comprobar el reflujo de varias líneas dentro del mismo bloque.",
    "Números 1234567890 y símbolos (paréntesis) también aparecen aquí.",
    "Pliego es una alternativa libre para editar documentos PDF sin salir del escritorio.",
    "Última frase de prueba con acentos: camión, niño, pingüino y corazón.",
]
FONTS = {
    "Helvetica": "Helvetica", "Times": "Times-Roman", "Courier": "Courier",
    "HelvB": "Helvetica-Bold", "TimesI": "Times-Italic", "CourierB": "Courier-Bold",
}


def esc(s):
    return s.replace("\\", "\\\\").replace("(", "\\(").replace(")", "\\)")


def wrap(text, width_chars):
    words, lines, cur = text.split(), [], ""
    for w in words:
        if cur and len(cur) + 1 + len(w) > width_chars:
            lines.append(cur)
            cur = w
        else:
            cur = f"{cur} {w}".strip()
    return lines + ([cur] if cur else [])


def text_ops(paras, font, size, x, y, mode, cols=1, width_chars=60):
    """mode: td | tm | tj (TJ con ajustes) | tstar (T*)."""
    out, lead = [], size * 1.3
    for pi, p in enumerate(paras):
        lines = wrap(p, width_chars)
        if mode == "tstar":
            out.append(f"BT /{font} {size} Tf {lead} TL {x} {y} Td")
            out.append(" T* ".join(f"({esc(l)}) Tj" if i else f"({esc(l)}) Tj" for i, l in enumerate(lines)))
            out.append("ET")
        else:
            for li, l in enumerate(lines):
                ly = y - li * lead
                if mode == "tm":
                    out.append(f"BT /{font} {size} Tf 1 0 0 1 {x} {ly} Tm ({esc(l)}) Tj ET")
                elif mode == "tj":
                    parts = l.split(" ")
                    arr = " ".join(f"({esc(w)}) -250" if i < len(parts) - 1 else f"({esc(w)})" for i, w in enumerate(parts))
                    out.append(f"BT /{font} {size} Tf {x} {ly} Td [{arr}] TJ ET")
                else:
                    out.append(f"BT /{font} {size} Tf {x} {ly} Td ({esc(l)}) Tj ET")
        y -= len(lines) * lead + size * 1.2
    return "\n".join(out)


def build(spec):
    objs = []

    def add(s):
        objs.append(s if isinstance(s, bytes) else s.encode("latin-1"))
        return len(objs)

    font_refs = {k: add(f"<< /Type /Font /Subtype /Type1 /BaseFont /{v} /Encoding /WinAnsiEncoding >>")
                 for k, v in FONTS.items()}
    xobj = ""
    extra = []
    if spec.get("image"):
        data = bytes([200, 220, 255] * 16)
        im = add(b"<< /Type /XObject /Subtype /Image /Width 4 /Height 4 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length 48 >>\nstream\n" + data + b"\nendstream")
        xobj = f"/XObject << /Im1 {im} 0 R >>"
        extra.append("q 120 0 0 90 400 620 cm /Im1 Do Q")
    if spec.get("vector"):
        extra.append("0.8 0.9 0.8 rg 72 90 200 30 re f 0 0 0 RG 1 w 72 80 m 272 80 l S")
    x0, y0 = spec.get("origin", (0, 0))
    if spec.get("rotate") in (90, 270):
        y0 -= 240  # la altura visible de estas páginas es 595
    streams = []
    body = text_ops(spec["paras"], spec["font"], spec["size"], 72 + x0, 760 + y0, spec["mode"],
                    width_chars=spec.get("chars", 60))
    if spec.get("columns") == 2:
        body += "\n" + text_ops(spec["paras2"], spec["font"], spec["size"], 330 + x0, 760 + y0, spec["mode"], width_chars=28)
    if spec.get("ocr"):
        body = "q 450 0 0 700 72 100 cm /Im1 Do Q\nBT 3 Tr\n" + body.replace("BT", "", 0) + "\nET"
        extra = []
    # Páginas giradas: el contenido se compensa para que el texto se vea derecho en pantalla (lo habitual).
    comp = {90: "0 1 -1 0 595 0", 180: "-1 0 0 -1 595 842", 270: "0 -1 1 0 0 842"}.get(spec.get("rotate"))
    if comp:
        body = f"q {comp} cm\n{body}\nQ"
    streams.append(body)
    streams.extend(extra)
    if not spec.get("split"):
        streams = ["\n".join(streams)]
    refs = []
    for s in streams:
        raw = s.encode("latin-1")
        if spec.get("flate"):
            z = zlib.compress(raw)
            refs.append(add(f"<< /Length {len(z)} /Filter /FlateDecode >>\nstream\n".encode() + z + b"\nendstream"))
        else:
            refs.append(add(f"<< /Length {len(raw)} >>\nstream\n".encode() + raw + b"\nendstream"))
    pages_id = len(objs) + 2
    fonts = " ".join(f"/{k} {r} 0 R" for k, r in font_refs.items())
    mb = spec.get("mediabox", "[0 0 595 842]")
    rot = f"/Rotate {spec['rotate']} " if spec.get("rotate") else ""
    crop = f"/CropBox {spec['crop']} " if spec.get("crop") else ""
    contents = refs[0] if len(refs) == 1 else None
    ctext = f"{contents} 0 R" if contents else "[" + " ".join(f"{r} 0 R" for r in refs) + "]"
    page = add(f"<< /Type /Page /Parent {pages_id} 0 R /MediaBox {mb} {crop}{rot}"
               f"/Resources << /Font << {fonts} >> {xobj} >> /Contents {ctext} >>")
    assert add(f"<< /Type /Pages /Kids [{page} 0 R] /Count 1 >>") == pages_id
    cat = add(f"<< /Type /Catalog /Pages {pages_id} 0 R >>")
    out = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
    offs = []
    for i, body in enumerate(objs, 1):
        offs.append(len(out))
        out += f"{i} 0 obj\n".encode() + body + b"\nendobj\n"
    xref = len(out)
    out += f"xref\n0 {len(objs)+1}\n0000000000 65535 f \n".encode()
    for o in offs:
        out += f"{o:010d} 00000 n \n".encode()
    out += f"trailer\n<< /Size {len(objs)+1} /Root {cat} 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode()
    return bytes(out)


def specs():
    out = []
    base = dict(paras=LOREM[:2], size=12, mode="td")
    for f in FONTS:  # 6: una por fuente (Td)
        out.append((f"fuente-{f.lower()}", {**base, "font": f}))
    for sz in (8, 10, 14, 18):  # 4: tamaños
        out.append((f"tamano-{sz}", {**base, "font": "Helvetica", "size": sz, "chars": int(700 / sz)}))
    for m in ("tm", "tj", "tstar"):  # 3: operadores (Word, LaTeX, impresoras)
        out.append((f"operador-{m}", {**base, "font": "Times", "mode": m}))
    for r in (90, 180, 270):  # 3: páginas giradas
        out.append((f"girada-{r}", {**base, "font": "Helvetica", "rotate": r}))
    out.append(("mediabox-desplazada", {**base, "font": "Helvetica", "mediabox": "[50 50 645 892]", "origin": (50, 50)}))
    out.append(("cropbox", {**base, "font": "Times", "crop": "[20 20 575 822]"}))
    out.append(("comprimido", {**base, "font": "Times", "flate": True}))
    out.append(("flujos-separados", {**base, "font": "Helvetica", "split": True, "vector": True}))
    out.append(("dos-columnas", {**base, "font": "Times", "columns": 2, "chars": 36, "paras": LOREM[:1], "paras2": LOREM[1:3]}))
    out.append(("con-imagen-y-vectores", {**base, "font": "Helvetica", "image": True, "vector": True, "flate": True}))
    out.append(("cinco-parrafos", {"font": "Times", "size": 11, "mode": "tm", "paras": LOREM}))
    out.append(("escaneo-ocr", {**base, "font": "Helvetica", "image": True, "ocr": True}))
    return out  # 6+4+3+3+1+1+1+1+1+1+1+1 = 24 -> se completan abajo


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "--blank":  # página en blanco para tools/gen_edit_cjk.sh
        pathlib.Path(sys.argv[2]).write_bytes(build({"font": "Helvetica", "size": 12, "mode": "td", "paras": []}))
        sys.exit(0)
    dest = pathlib.Path(__file__).resolve().parent.parent / "tests" / "corpus" / "edit" / "corpus"
    dest.mkdir(parents=True, exist_ok=True)
    items = specs()
    # Completar hasta 27 con combinaciones de tamaño y operador.
    for i, (m, f) in enumerate([("tj", "Courier"), ("tm", "HelvB"), ("tstar", "TimesI")]):
        items.append((f"mezcla-{i+1}", {"font": f, "size": 13, "mode": m, "paras": LOREM[2:4], "flate": True}))
    assert len(items) == 27, len(items)
    for name, spec in items:
        (dest / f"{name}.pdf").write_bytes(build(spec))
    print(len(items), "PDFs en", dest)
