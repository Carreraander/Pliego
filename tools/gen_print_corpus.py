"""Genera los PDFs de prueba de tests/corpus/print/ (producción de impresión).
Sin dependencias. Uso: python tools/gen_print_corpus.py

- defectos.pdf: fuente sin incrustar, imagen RGB a 50 ppp, línea de 0,05 pt, RGB, 3 colores directos, sin TrimBox.
- limpio.pdf: Type3 (cuenta como incrustada), solo CMYK, líneas de 0,5 pt, con TrimBox.
- cmyk-*.pdf: página llena con un color CMYK conocido (las planchas esperadas se calculan a mano).
- directo.pdf: un color directo (Separation «PANTONE 185 C») sobre CMYK.
"""
import pathlib

def stream(d, data):
    if isinstance(data, str):
        data = data.encode("latin-1")
    return b"<< " + d.encode() + b" /Length " + str(len(data)).encode() + b" >>\nstream\n" + data + b"\nendstream"

def build(content, resources="", extra=None, page_extra="", media="[0 0 200 200]"):
    """extra: lista de cuerpos de objetos (bytes o str) que ocupan los números 4, 5, ..."""
    objs = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        f"<< /Type /Page /Parent 2 0 R /MediaBox {media} /Resources << {resources} >> /Contents 4 0 R {page_extra} >>",
        stream("", content),
    ] + (extra or [])
    out = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
    offs = []
    for i, body in enumerate(objs, 1):
        offs.append(len(out))
        if isinstance(body, str):
            body = body.encode("latin-1")
        out += f"{i} 0 obj\n".encode() + body + b"\nendobj\n"
    xref = len(out)
    out += f"xref\n0 {len(objs)+1}\n0000000000 65535 f \n".encode()
    for o in offs:
        out += f"{o:010d} 00000 n \n".encode()
    out += f"trailer\n<< /Size {len(objs)+1} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode()
    return bytes(out)

def defectos():
    img = stream("/Type /XObject /Subtype /Image /Width 10 /Height 10 /ColorSpace /DeviceRGB /BitsPerComponent 8",
                 bytes(range(200, 230)) * 10)
    sep = lambda n: f"[/Separation /{n} /DeviceCMYK << /FunctionType 2 /Domain [0 1] /C0 [0 0 0 0] /C1 [0 1 1 0] /N 1 >>]"
    content = (
        "BT /F1 18 Tf 20 170 Td (Texto sin incrustar) Tj ET\n"
        "0.05 w 0 0 1 RG 20 150 m 180 150 l S\n"
        "1 0 0 rg 20 100 40 30 re f\n"
        "q 144 0 0 144 40 0 cm /Im1 Do Q\n"
        "/CS1 cs 1 scn 100 100 30 30 re f\n/CS2 cs 1 scn 140 100 30 30 re f\n/CS3 cs 1 scn 100 60 30 30 re f\n"
    )
    res = ("/Font << /F1 5 0 R >> /XObject << /Im1 6 0 R >> /ColorSpace << /CS1 "
           + sep("PANTONE#20185#20C") + " /CS2 " + sep("PANTONE#20300#20C") + " /CS3 " + sep("Barniz") + " >>")
    # 144 pt = 2 pulgadas para 10 px -> 5 ppp.
    return build(content, res, ["<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>", img])

def limpio():
    glyph = stream("", "1000 0 0 0 750 750 d1 0 0 750 750 re f")
    f3 = ("<< /Type /Font /Subtype /Type3 /FontBBox [0 0 750 750] /FontMatrix [0.001 0 0 0.001 0 0] "
          "/CharProcs << /a 5 0 R >> /Encoding << /Type /Encoding /Differences [97 /a] >> /FirstChar 97 /LastChar 97 /Widths [1000] >>")
    content = "BT /F1 18 Tf 0 0 0 1 k 20 170 Td (aaa) Tj ET\n0.5 w 1 0 0 0 K 20 150 m 180 150 l S\n0 1 0 0 k 20 100 40 30 re f\n"
    return build(content, "/Font << /F1 6 0 R >>", [glyph, f3], page_extra="/TrimBox [10 10 190 190]")

def cmyk(c, m, y, k):
    return build(f"{c} {m} {y} {k} k 0 0 200 200 re f\n", page_extra="/TrimBox [10 10 190 190]")

def directo():
    sep = "[/Separation /Barniz /DeviceCMYK << /FunctionType 2 /Domain [0 1] /C0 [0 0 0 0] /C1 [0 1 1 0] /N 1 >>]"
    content = "0 0 0 0.5 k 0 0 200 200 re f\n/CS1 cs 1 scn 50 50 100 100 re f\n"
    return build(content, f"/ColorSpace << /CS1 {sep} >>", page_extra="/TrimBox [10 10 190 190]")

if __name__ == "__main__":
    d = pathlib.Path(__file__).resolve().parent.parent / "tests" / "corpus" / "print"
    d.mkdir(parents=True, exist_ok=True)
    files = {
        "defectos.pdf": defectos(), "limpio.pdf": limpio(), "directo.pdf": directo(),
        "cmyk-cian.pdf": cmyk(1, 0, 0, 0), "cmyk-magenta.pdf": cmyk(0, 1, 0, 0),
        "cmyk-amarillo.pdf": cmyk(0, 0, 1, 0), "cmyk-negro.pdf": cmyk(0, 0, 0, 1),
        "cmyk-mezcla.pdf": cmyk(0.5, 0.25, 0.75, 0.1),
    }
    for name, data in files.items():
        (d / name).write_bytes(data)
        print(d / name)
