#!/usr/bin/env python3
"""Genera un PDF de texto de N páginas (por defecto 500) para los benchmarks.

Uso: python tools/gen_bench_pdf.py [--pages 500] [--out tests/output/bench-500.pdf]
Sin dependencias: escribe PDF 1.4 a mano (Helvetica, texto, sin imágenes). Es determinista.
"""
import argparse
from pathlib import Path


def build(pages: int) -> bytes:
    objs = []  # contenido de cada objeto, índice = número - 1

    def add(body: bytes) -> int:
        objs.append(body)
        return len(objs)

    add(b"")  # 1: catálogo (se rellena después)
    add(b"")  # 2: árbol de páginas
    font = add(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")
    kids = []
    for p in range(pages):
        lines = [b"BT /F1 11 Tf 56 780 Td 14 TL"]
        for i in range(52):
            text = f"Página {p + 1}, línea {i + 1}: pliego es un visor y editor de PDF libre con motor MuPDF."
            text = text.encode("cp1252", "replace").replace(b"\\", b"\\\\").replace(b"(", b"\\(").replace(b")", b"\\)")
            lines.append(b"(" + text + b") Tj T*")
        lines.append(b"ET")
        stream = b"\n".join(lines)
        content = add(b"<< /Length %d >>\nstream\n" % len(stream) + stream + b"\nendstream")
        page = add(
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 %d 0 R >> >> /Contents %d 0 R >>"
            % (font, content)
        )
        kids.append(b"%d 0 R" % page)
    objs[0] = b"<< /Type /Catalog /Pages 2 0 R >>"
    objs[1] = b"<< /Type /Pages /Count %d /Kids [%s] >>" % (pages, b" ".join(kids))
    out = bytearray(b"%PDF-1.4\n")
    offsets = []
    for n, body in enumerate(objs, 1):
        offsets.append(len(out))
        out += b"%d 0 obj\n" % n + body + b"\nendobj\n"
    xref = len(out)
    out += b"xref\n0 %d\n0000000000 65535 f \n" % (len(objs) + 1)
    for off in offsets:
        out += b"%010d 00000 n \n" % off
    out += b"trailer\n<< /Size %d /Root 1 0 R >>\nstartxref\n%d\n%%%%EOF\n" % (len(objs) + 1, xref)
    return bytes(out)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--pages", type=int, default=500)
    ap.add_argument("--out", type=Path, default=Path("tests/output/bench-500.pdf"))
    args = ap.parse_args()
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_bytes(build(args.pages))
    print(f"{args.out} ({args.pages} páginas, {args.out.stat().st_size // 1024} KB)")


if __name__ == "__main__":
    main()
