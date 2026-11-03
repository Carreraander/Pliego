"""Genera los pares de PDFs de tests/corpus/compare/ (pequeños, sin datos reales).
Sin dependencias. Uso: python tools/gen_compare_corpus.py"""
import pathlib
import zlib

PARA1 = ["El contrato entra en vigor el primer dia del mes siguiente", "a la firma de ambas partes ante testigos."]
PARA2 = ["Las obligaciones de pago se realizaran mediante transferencia", "bancaria en un plazo maximo de treinta dias."]
PARA3 = ["Cualquier disputa se resolvera ante los tribunales", "competentes de la ciudad de Madrid."]
NEW_PARA = ["Anexo: ambas partes aceptan expresamente la clausula adicional", "sobre confidencialidad de la informacion intercambiada."]


def esc(s):
    return s.replace("\\", "\\\\").replace("(", "\\(").replace(")", "\\)")


def page(lines=(), images=(), size=12, font="F1", extra=()):
    """lines: lista de (x, y, texto[, tamano, fuente]); images: lista de (x, y, ancho, alto, (r, g, b))."""
    return {"lines": list(lines), "images": list(images), "extra": list(extra)}


def para(x, y, lines, size=12, font="F1"):
    return [(x, y - i * (size + 4), t, size, font) for i, t in enumerate(lines)]


def build(pages):
    objs = []

    def add(body):
        objs.append(body)
        return len(objs)

    f1 = add(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>")
    f2 = add(b"<< /Type /Font /Subtype /Type1 /BaseFont /Times-Roman /Encoding /WinAnsiEncoding >>")
    fonts = {"F1": f1, "F2": f2}
    page_data = []
    for p in pages:
        ops = []
        for ln in p["lines"]:
            x, y, t = ln[:3]
            size = ln[3] if len(ln) > 3 else 12
            font = ln[4] if len(ln) > 4 else "F1"
            ops.append(f"BT /{font} {size} Tf {x} {y} Td ({esc(t)}) Tj ET")
        xobjs = []
        for k, (x, y, w, h, rgb) in enumerate(p["images"]):
            raw = bytes(rgb) * 16
            data = zlib.compress(raw)
            im = add(
                b"<< /Type /XObject /Subtype /Image /Width 4 /Height 4 /ColorSpace /DeviceRGB "
                b"/BitsPerComponent 8 /Filter /FlateDecode /Length %d >>\nstream\n" % len(data) + data + b"\nendstream"
            )
            xobjs.append(f"/Im{k} {im} 0 R")
            ops.append(f"q {w} 0 0 {h} {x} {y} cm /Im{k} Do Q")
        stream = "\n".join(ops).encode("latin-1")
        c = add(b"<< /Length %d >>\nstream\n" % len(stream) + stream + b"\nendstream")
        page_data.append((c, xobjs))
    pages_id = len(objs) + len(pages) + 1
    kids = []
    for c, xobjs in page_data:
        res = "<< /Font << /F1 %d 0 R /F2 %d 0 R >> /XObject << %s >> >>" % (f1, f2, " ".join(xobjs))
        kids.append(add(f"<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 595 842] /Resources {res} /Contents {c} 0 R >>".encode()))
    assert add(f"<< /Type /Pages /Kids [{' '.join(f'{k} 0 R' for k in kids)}] /Count {len(kids)} >>".encode()) == pages_id
    cat = add(f"<< /Type /Catalog /Pages {pages_id} 0 R >>".encode())
    out = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
    offs = []
    for i, body in enumerate(objs, 1):
        offs.append(len(out))
        out += b"%d 0 obj\n" % i + body + b"\nendobj\n"
    xref = len(out)
    out += b"xref\n0 %d\n0000000000 65535 f \n" % (len(objs) + 1)
    for o in offs:
        out += b"%010d 00000 n \n" % o
    out += b"trailer\n<< /Size %d /Root %d 0 R >>\nstartxref\n%d\n%%%%EOF\n" % (len(objs) + 1, cat, xref)
    return bytes(out)


def body(title, paras):
    lines = [(72, 780, title, 18)]
    y = 740
    for p in paras:
        lines += para(72, y, p)
        y -= 60
    return lines


def main():
    out = pathlib.Path(__file__).resolve().parent.parent / "tests" / "corpus" / "compare"
    out.mkdir(parents=True, exist_ok=True)
    p2_other = ["Segunda hoja del documento con datos del anexo tecnico", "y la lista de entregables acordados."]

    def write(name, pages):
        (out / name).write_bytes(build(pages))

    base = [page(body("Contrato de servicios", [PARA1, PARA2, PARA3])), page(para(72, 760, p2_other))]
    write("base.pdf", base)
    # palabra cambiada en la página 1 (treinta -> sesenta)
    changed = [PARA2[0], PARA2[1].replace("treinta", "sesenta")]
    write("palabra.pdf", [page(body("Contrato de servicios", [PARA1, changed, PARA3])), base[1]])
    # párrafo insertado al final de la página 1
    write("parrafo.pdf", [page(body("Contrato de servicios", [PARA1, PARA2, PARA3, NEW_PARA])), base[1]])
    # página insertada en medio (nueva página 2)
    mid = page(para(72, 760, ["Pagina nueva insertada entre las dos originales", "con texto totalmente distinto del resto."]))
    write("pagina.pdf", [base[0], mid, base[1]])
    # formato: el título pasa a 24 pt y Times
    write("formato.pdf", [page([(72, 780, "Contrato de servicios", 24, "F2")] + body("", [PARA1, PARA2, PARA3])[1:]), base[1]])
    # imagen movida
    img = [page(para(72, 760, PARA1), images=[(72, 500, 120, 120, (200, 30, 30))])]
    img2 = [page(para(72, 760, PARA1), images=[(360, 300, 120, 120, (200, 30, 30))])]
    write("imagen_a.pdf", img)
    write("imagen_b.pdf", img2)


if __name__ == "__main__":
    main()
