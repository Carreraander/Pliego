"""Genera tests/corpus/organize/: a.pdf y b.pdf (3 y 2 páginas con el texto "<X> p<N>" y un marcador por página),
form1.pdf y form2.pdf (campos de texto, con el nombre "nombre" repetido) y links.pdf (enlace interno y externo).
Sin dependencias. Uso: python tools/gen_organize_corpus.py"""
import pathlib


def build(label, pages):
    objs = ["<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"]
    pages_id = 2 + pages * 2
    for i in range(pages):
        stream = f"BT /F1 24 Tf 72 760 Td ({label} p{i + 1}) Tj ET"
        objs.append(f"<< /Length {len(stream)} >>\nstream\n{stream}\nendstream")
        objs.append(f"<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 595 842] "
                    f"/Resources << /Font << /F1 1 0 R >> >> /Contents {2 + 2 * i} 0 R >>")
    kids = " ".join(f"{3 + 2 * i} 0 R" for i in range(pages))
    objs.append(f"<< /Type /Pages /Kids [{kids}] /Count {pages} >>")
    outlines = len(objs) + 1
    first, last = outlines + 1, outlines + pages
    objs.append(f"<< /Type /Outlines /First {first} 0 R /Last {last} 0 R /Count {pages} >>")
    for i in range(pages):
        prev = f"/Prev {first + i - 1} 0 R" if i else ""
        nxt = f"/Next {first + i + 1} 0 R" if i + 1 < pages else ""
        objs.append(f"<< /Title ({label} m{i + 1}) /Parent {outlines} 0 R {prev} {nxt} /Dest [{3 + 2 * i} 0 R /Fit] >>")
    catalog = len(objs) + 1
    objs.append(f"<< /Type /Catalog /Pages {pages_id} 0 R /Outlines {outlines} 0 R >>")
    out = bytearray(b"%PDF-1.7\n")
    offsets = []
    for i, body in enumerate(objs, 1):
        offsets.append(len(out))
        out += f"{i} 0 obj\n{body}\nendobj\n".encode("latin-1")
    xref = len(out)
    out += f"xref\n0 {len(objs) + 1}\n0000000000 65535 f \n".encode()
    for off in offsets:
        out += f"{off:010d} 00000 n \n".encode()
    out += f"trailer\n<< /Size {len(objs) + 1} /Root {catalog} 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode()
    return bytes(out)


def finish(objs, catalog):
    out = bytearray(b"%PDF-1.7\n")
    offsets = []
    for i, body in enumerate(objs, 1):
        offsets.append(len(out))
        out += f"{i} 0 obj\n{body}\nendobj\n".encode("latin-1")
    xref = len(out)
    out += f"xref\n0 {len(objs) + 1}\n0000000000 65535 f \n".encode()
    for off in offsets:
        out += f"{off:010d} 00000 n \n".encode()
    out += f"trailer\n<< /Size {len(objs) + 1} /Root {catalog} 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode()
    return bytes(out)


def build_form(names):
    """Una página por campo de texto."""
    objs = ["<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"]
    n = len(names)
    pages_id = 2 + n * 3
    ids = []
    for i, name in enumerate(names):
        stream = f"BT /F1 18 Tf 72 760 Td (Form {i + 1}) Tj ET"
        objs.append(f"<< /Length {len(stream)} >>\nstream\n{stream}\nendstream")
        c = len(objs)
        p, w = len(objs) + 1, len(objs) + 2
        objs.append(f"<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 595 842] "
                    f"/Resources << /Font << /F1 1 0 R >> >> /Contents {c} 0 R /Annots [{w} 0 R] >>")
        objs.append(f"<< /Type /Annot /Subtype /Widget /FT /Tx /T ({name}) /V (valor{i}) /DA (/F1 12 Tf 0 g) "
                    f"/Rect [72 600 300 630] /P {p} 0 R /F 4 >>")
        ids.append((p, w))
    objs.append("<< /Type /Pages /Kids [%s] /Count %d >>" % (" ".join(f"{p} 0 R" for p, _ in ids), n))
    objs.append("<< /Fields [%s] /DA (/F1 12 Tf 0 g) /DR << /Font << /F1 1 0 R >> >> >>" % " ".join(f"{w} 0 R" for _, w in ids))
    objs.append(f"<< /Type /Catalog /Pages {pages_id} 0 R /AcroForm {len(objs)} 0 R >>")
    return finish(objs, len(objs))


def build_links():
    """Dos páginas; la 1 tiene un enlace interno a la 2 y otro a una web."""
    objs = ["<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"]
    for i in range(2):
        stream = f"BT /F1 18 Tf 72 760 Td (Enlaces p{i + 1}) Tj ET"
        objs.append(f"<< /Length {len(stream)} >>\nstream\n{stream}\nendstream")
    # 4 = página 1, 5 = página 2, 6 = árbol, 7 y 8 = enlaces
    objs.append("<< /Type /Page /Parent 6 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 1 0 R >> >> "
                "/Contents 2 0 R /Annots [7 0 R 8 0 R] >>")
    objs.append("<< /Type /Page /Parent 6 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 1 0 R >> >> /Contents 3 0 R >>")
    objs.append("<< /Type /Pages /Kids [4 0 R 5 0 R] /Count 2 >>")
    objs.append("<< /Type /Annot /Subtype /Link /Rect [72 700 300 730] /Border [0 0 0] /Dest [5 0 R /Fit] >>")
    objs.append("<< /Type /Annot /Subtype /Link /Rect [72 600 300 630] /Border [0 0 0] "
                "/A << /S /URI /URI (https://example.org/) >> >>")
    objs.append("<< /Type /Catalog /Pages 6 0 R >>")
    return finish(objs, len(objs))


dest = pathlib.Path(__file__).resolve().parent.parent / "tests" / "corpus" / "organize"
dest.mkdir(parents=True, exist_ok=True)
(dest / "a.pdf").write_bytes(build("A", 3))
(dest / "b.pdf").write_bytes(build("B", 2))
(dest / "form1.pdf").write_bytes(build_form(["nombre", "email"]))
(dest / "form2.pdf").write_bytes(build_form(["nombre"]))
(dest / "links.pdf").write_bytes(build_links())
