"""Genera tests/corpus/organize/{a,b}.pdf: PDFs de 3 y 2 páginas con el texto "<X> p<N>" y un marcador por página.
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


dest = pathlib.Path(__file__).resolve().parent.parent / "tests" / "corpus" / "organize"
dest.mkdir(parents=True, exist_ok=True)
(dest / "a.pdf").write_bytes(build("A", 3))
(dest / "b.pdf").write_bytes(build("B", 2))
