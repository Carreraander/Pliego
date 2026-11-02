"""Genera tests/corpus/basic/hola.pdf: 2 páginas A4 con texto y un marcador por página.
Sin dependencias. Uso: python tools/gen_basic_corpus.py"""
import pathlib

def build():
    objs = []
    def add(s):
        objs.append(s)
        return len(objs)
    font = add("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")
    contents = []
    for i, txt in enumerate(["Hola Pliego pagina uno", "Segunda pagina de prueba Pliego"]):
        stream = f"BT /F1 24 Tf 72 760 Td ({txt}) Tj ET"
        contents.append(add(f"<< /Length {len(stream)} >>\nstream\n{stream}\nendstream"))
    pages_id = len(objs) + 3  # páginas: n+1, n+2; árbol de páginas: n+3
    page_ids = []
    for c in contents:
        page_ids.append(add(f"<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 595 842] "
                            f"/Resources << /Font << /F1 {font} 0 R >> >> /Contents {c} 0 R >>"))
    kids = " ".join(f"{p} 0 R" for p in page_ids)
    assert add(f"<< /Type /Pages /Kids [{kids}] /Count {len(page_ids)} >>") == pages_id
    outlines_id = len(objs) + 1
    o1, o2 = outlines_id + 1, outlines_id + 2
    add(f"<< /Type /Outlines /First {o1} 0 R /Last {o2} 0 R /Count 2 >>")
    add(f"<< /Title (Inicio) /Parent {outlines_id} 0 R /Next {o2} 0 R /Dest [{page_ids[0]} 0 R /Fit] >>")
    add(f"<< /Title (Segunda) /Parent {outlines_id} 0 R /Prev {o1} 0 R /Dest [{page_ids[1]} 0 R /Fit] >>")
    info = add("<< /Title (Documento de prueba) /Author (Pliego) >>")
    catalog = add(f"<< /Type /Catalog /Pages {pages_id} 0 R /Outlines {outlines_id} 0 R >>")
    out = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
    offsets = []
    for i, body in enumerate(objs, 1):
        offsets.append(len(out))
        out += f"{i} 0 obj\n{body}\nendobj\n".encode("latin-1")
    xref = len(out)
    out += f"xref\n0 {len(objs)+1}\n0000000000 65535 f \n".encode()
    for off in offsets:
        out += f"{off:010d} 00000 n \n".encode()
    out += f"trailer\n<< /Size {len(objs)+1} /Root {catalog} 0 R /Info {info} 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode()
    return bytes(out)

if __name__ == "__main__":
    p = pathlib.Path(__file__).resolve().parent.parent / "tests" / "corpus" / "basic" / "hola.pdf"
    p.write_bytes(build())
    print(p)
