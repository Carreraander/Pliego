"""Genera tests/corpus/outline/hostil.pdf: 2 páginas con marcadores hostiles (sin /Title, destino con nombre,
ciclos por /Next y por /First). Sin dependencias. Uso: python tools/gen_outline_corpus.py"""
import pathlib


def build():
    objs = []

    def add(s):
        objs.append(s)
        return len(objs)

    font = add("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")
    contents = [add(f"<< /Length 0 >>\nstream\n\nendstream") for _ in range(2)]
    pages_id = len(objs) + 3
    pages = [add(f"<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 200 200] "
                 f"/Resources << /Font << /F1 {font} 0 R >> >> /Contents {c} 0 R >>") for c in contents]
    assert add(f"<< /Type /Pages /Kids [{pages[0]} 0 R {pages[1]} 0 R] /Count 2 >>") == pages_id
    root = len(objs) + 1
    a, b, c, d = root + 1, root + 2, root + 3, root + 4
    add(f"<< /Type /Outlines /First {a} 0 R /Last {c} 0 R /Count 4 >>")
    add(f"<< /Title (Inicio) /Parent {root} 0 R /Next {b} 0 R /Dest [{pages[0]} 0 R /Fit] >>")
    # Sin /Title y con destino con nombre.
    add(f"<< /Parent {root} 0 R /Next {c} 0 R /A << /S /GoTo /D /destB >> >>")
    # Ciclo: /Next vuelve al primero y el hijo apunta de vuelta a este nodo con /First.
    add(f"<< /Title (Ciclo) /Parent {root} 0 R /Next {a} 0 R /First {d} 0 R >>")
    add(f"<< /Title (Hijo) /Parent {c} 0 R /First {c} 0 R /Dest [{pages[1]} 0 R /Fit] >>")
    catalog = add(f"<< /Type /Catalog /Pages {pages_id} 0 R /Outlines {root} 0 R "
                  f"/Dests << /destB [{pages[1]} 0 R /Fit] >> >>")
    out = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
    offsets = []
    for i, body in enumerate(objs, 1):
        offsets.append(len(out))
        out += f"{i} 0 obj\n{body}\nendobj\n".encode("latin-1")
    xref = len(out)
    out += f"xref\n0 {len(objs)+1}\n0000000000 65535 f \n".encode()
    for off in offsets:
        out += f"{off:010d} 00000 n \n".encode()
    out += (f"trailer\n<< /Size {len(objs)+1} /Root {catalog} 0 R >>\nstartxref\n{xref}\n%%EOF\n").encode()
    return bytes(out)


if __name__ == "__main__":
    p = pathlib.Path(__file__).resolve().parent.parent / "tests" / "corpus" / "outline" / "hostil.pdf"
    p.write_bytes(build())
    print(p)
