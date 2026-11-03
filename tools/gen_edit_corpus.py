"""Genera tests/corpus/edit/parrafos.pdf (A4 con dos párrafos y un recuadro gráfico), imagenes.pdf (dos imágenes
colocadas con matrices distintas) y rojo.png / azul.png (imágenes de 8x8).
Sin dependencias. Uso: python tools/gen_edit_corpus.py"""
import pathlib
import struct
import zlib


def build(with_images=False):
    objs = []

    def add(s):
        objs.append(s)
        return len(objs)

    f1 = add("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>")
    f2 = add("<< /Type /Font /Subtype /Type1 /BaseFont /Times-Roman /Encoding /WinAnsiEncoding >>")
    xobj = ""
    if with_images:
        def img(rgb):
            data = bytes(rgb) * 16
            return add(f"<< /Type /XObject /Subtype /Image /Width 4 /Height 4 /ColorSpace /DeviceRGB "
                       f"/BitsPerComponent 8 /Length {len(data)} >>\nstream\n" + data.decode("latin-1") + "\nendstream")
        i1, i2 = img((255, 0, 0)), img((0, 0, 255))
        xobj = f"/XObject << /Im1 {i1} 0 R /Im2 {i2} 0 R >> "
    lines = [
        "BT /F1 12 Tf 14 TL 72 760 Td (Primer parrafo de prueba para editar.) Tj T* (Segunda linea del primer parrafo.) Tj T* (Tercera linea del primer parrafo.) Tj ET",
        "BT /F2 14 Tf 18 TL 72 600 Td (Este es otro bloque en tipografia con remate.) Tj T* (Debe quedar intacto al editar el primero.) Tj ET",
        "0.9 0.2 0.2 rg 72 500 200 40 re f",
    ]
    if with_images:
        lines = ["q 100 0 0 80 72 600 cm /Im1 Do Q", "q 0.5 0 0 0.5 0 0 cm 120 0 0 120 400 1000 cm /Im2 Do Q"]
    stream = "\n".join(lines)
    c = add(f"<< /Length {len(stream)} >>\nstream\n{stream}\nendstream")
    pages_id = len(objs) + 2
    page = add(f"<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 595 842] "
               f"/Resources << /Font << /F1 {f1} 0 R /F2 {f2} 0 R >> {xobj}>> /Contents {c} 0 R >>")
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


def png(rgb, size=8):
    def chunk(t, d):
        c = struct.pack(">I", len(d)) + t + d
        return c + struct.pack(">I", zlib.crc32(t + d))
    raw = b"".join(b"\x00" + bytes(rgb) * size for _ in range(size))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))


if __name__ == "__main__":
    dest = pathlib.Path(__file__).resolve().parent.parent / "tests" / "corpus" / "edit" / "parrafos.pdf"
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_bytes(build())
    (dest.parent / "imagenes.pdf").write_bytes(build(True))
    (dest.parent / "rojo.png").write_bytes(png((255, 0, 0)))
    (dest.parent / "azul.png").write_bytes(png((0, 0, 255)))
    print(dest)
