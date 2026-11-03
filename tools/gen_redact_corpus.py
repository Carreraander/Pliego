"""Genera tests/corpus/redact/datos.pdf: datos personales ficticios, una imagen y un vector, para probar la redacción.
Todo es inventado (ejemplos oficiales o calculados). Sin dependencias. Uso: python tools/gen_redact_corpus.py"""
import pathlib

LINES = [
    "Nombre: Ana Lopez   DNI 12345678Z",
    "NIE X1234567L   CIF A58818501",
    "IBAN ES91 2100 0418 4502 0005 1332",
    "Correo ana.lopez@ejemplo.es   Tel +34 612 345 678",
    "Fecha de nacimiento 12/03/1985",
    "DNI invalido 12345678A y texto normal conservado",
]

def build():
    objs = []
    def add(s):
        objs.append(s)
        return len(objs)
    font = add("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")
    # Imagen 4x4 RGB gris oscuro (sin compresión).
    img_data = bytes([40, 40, 40] * 16).decode("latin-1")
    img = add(f"<< /Type /XObject /Subtype /Image /Width 4 /Height 4 /ColorSpace /DeviceRGB /BitsPerComponent 8 "
              f"/Length {len(img_data)} >>\nstream\n{img_data}\nendstream")
    ops = ["BT /F1 11 Tf"]
    y = 780
    for i, t in enumerate(LINES):
        ops.append(f"1 0 0 1 72 {y - 14 * i} Tm ({t}) Tj")
    ops.append("ET")
    # Imagen en [72,300]-[172,400] y rectángulo rojo en [300,300]-[400,400] (coordenadas PDF, y hacia arriba).
    ops.append("q 100 0 0 100 72 300 cm /Im1 Do Q")
    ops.append("q 1 0 0 rg 300 300 100 100 re f Q")
    s1 = "\n".join(ops)
    c1 = add(f"<< /Length {len(s1)} >>\nstream\n{s1}\nendstream")
    s2 = "BT /F1 11 Tf 72 780 Td (Segunda pagina con DNI 12345678Z) Tj ET"
    c2 = add(f"<< /Length {len(s2)} >>\nstream\n{s2}\nendstream")
    pages_id = len(objs) + 3
    res = f"/Resources << /Font << /F1 {font} 0 R >> /XObject << /Im1 {img} 0 R >> >>"
    p1 = add(f"<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 595 842] {res} /Contents {c1} 0 R >>")
    p2 = add(f"<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 595 842] {res} /Contents {c2} 0 R >>")
    assert add(f"<< /Type /Pages /Kids [{p1} 0 R {p2} 0 R] /Count 2 >>") == pages_id
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
    p = pathlib.Path(__file__).resolve().parent.parent / "tests" / "corpus" / "redact" / "datos.pdf"
    p.write_bytes(build())
    print(p)
