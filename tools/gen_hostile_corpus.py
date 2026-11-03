"""Genera tests/corpus/worker/tiling-hang.pdf: patrón de mosaico con pasos minúsculos (XStep/YStep = 0.001)
que obliga a MuPDF a repetir el motivo millones de veces. Sin dependencias.
Uso: python tools/gen_hostile_corpus.py"""
import pathlib


def build():
    objs = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << /Pattern << /P1 4 0 R >> >> /Contents 5 0 R >>",
        "<< /PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 10 10] /XStep 0.001 /YStep 0.001 "
        "/Resources << >> /Length 19 >>\nstream\nq 0 0 10 10 re f Q\nendstream",
        "<< /Length 41 >>\nstream\nq /Pattern cs /P1 scn 0 0 200 200 re f Q\nendstream",
    ]
    out = bytearray(b"%PDF-1.7\n")
    offsets = []
    for i, body in enumerate(objs, 1):
        offsets.append(len(out))
        out += f"{i} 0 obj\n{body}\nendobj\n".encode("latin-1")
    xref = len(out)
    out += f"xref\n0 {len(objs)+1}\n0000000000 65535 f \n".encode()
    for off in offsets:
        out += f"{off:010d} 00000 n \n".encode()
    out += f"trailer\n<< /Size {len(objs)+1} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode()
    return bytes(out)


if __name__ == "__main__":
    p = pathlib.Path(__file__).resolve().parent.parent / "tests" / "corpus" / "worker" / "tiling-hang.pdf"
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_bytes(build())
    print(p)
