"""Genera tests/corpus/edit/parrafos.pdf: una página A4 con dos párrafos (Helvetica y Times) y un recuadro gráfico.
Sin dependencias. Uso: python tools/gen_edit_corpus.py"""
import pathlib


def build():
    objs = []

    def add(s):
        objs.append(s)
        return len(objs)

    f1 = add("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>")
    f2 = add("<< /Type /Font /Subtype /Type1 /BaseFont /Times-Roman /Encoding /WinAnsiEncoding >>")
    lines = [
        "BT /F1 12 Tf 14 TL 72 760 Td (Primer parrafo de prueba para editar.) Tj T* (Segunda linea del primer parrafo.) Tj T* (Tercera linea del primer parrafo.) Tj ET",
        "BT /F2 14 Tf 18 TL 72 600 Td (Este es otro bloque en tipografia con remate.) Tj T* (Debe quedar intacto al editar el primero.) Tj ET",
        "0.9 0.2 0.2 rg 72 500 200 40 re f",
    ]
    stream = "\n".join(lines)
    c = add(f"<< /Length {len(stream)} >>\nstream\n{stream}\nendstream")
    pages_id = len(objs) + 2
    page = add(f"<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 595 842] "
               f"/Resources << /Font << /F1 {f1} 0 R /F2 {f2} 0 R >> >> /Contents {c} 0 R >>")
    assert add(f"<< /Type /Pages /Kids [{page} 0 R] /Count 1 >>") == pages_id
    catalog = add(f"<< /Type /Catalog /Pages {pages_id} 0 R >>")
    out = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
    offsets = []
    for i, body in enumerate(objs, 1):
        offsets.append(len(out))
        out += f"{i} 0 obj\n{body}\nendobj\n".encode("latin-1")
    xref = len(out)
    out += f"xref\n0 {len(objs)+1}\n0000000000 65535 f \n".encode()
    for off in offsets:
        out += f"{off:010d} 00000 n \n".encode()
    out += f"trailer\n<< /Size {len(objs)+1} /Root {catalog} 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode()
    return bytes(out)


if __name__ == "__main__":
    dest = pathlib.Path(__file__).resolve().parent.parent / "tests" / "corpus" / "edit" / "parrafos.pdf"
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_bytes(build())
    print(dest)
