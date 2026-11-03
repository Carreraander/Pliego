"""Genera los PDFs grandes para medir W08 (no se suben al repo): text_a.pdf/text_b.pdf (500 páginas) y scan.pdf
(70 escaneos A4 a 300 ppp, ~210 MB). Uso: python tools/gen_w08_bench.py [carpeta_salida]. Requiere Pillow y numpy."""
import os, pathlib
os.chdir(pathlib.Path(__import__("sys").argv[1]) if len(__import__("sys").argv) > 1 else ".")
import sys, io, random, numpy as np
from PIL import Image
sys.path.insert(0, str(__import__("pathlib").Path(__file__).resolve().parent))
import gen_compare_corpus as g

def words(rng, n):
    vocab = "contrato servicio parte clausula plazo pago entrega garantia anexo informe cliente proveedor importe fecha firma".split()
    return " ".join(rng.choice(vocab) for _ in range(n))

def text_doc(seed, pages, edit=False):
    rng = random.Random(seed)
    out = []
    for p in range(pages):
        lines = []
        for i in range(45):
            lines.append((50, 800 - i * 16, f"{p}-{i} " + words(rng, 12)))
        if edit and p % 25 == 3:
            lines[10] = (50, 640, "TEXTO MODIFICADO en esta linea de la pagina")
        out.append(g.page(lines))
    return g.build(out)

open("text_a.pdf", "wb").write(text_doc(1, 500))
open("text_b.pdf", "wb").write(text_doc(1, 500, edit=True))

# Escaneo: 70 páginas A4 a 300 ppp, JPEG gris con ruido (~3 MB cada una)
rng = np.random.default_rng(0)
objs = []
def add(b): objs.append(b); return len(objs)
jpegs = []
for k in range(70):
    a = (225 + rng.integers(0, 20, (3508, 2480))).astype(np.uint8)
    for y in range(400, 3100, 60):
        a[y:y+30, 300:2180] = rng.integers(30, 70, (30, 1880))
    b = io.BytesIO(); Image.fromarray(a, "L").save(b, "JPEG", quality=92); jpegs.append(b.getvalue())
f1 = add(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")
P = 70
page_ids = []
pages_id = 2 * P + 1 + 1 + P  # placeholder recomputed below
data = []
for p in range(P):
    j = jpegs[p]
    im = add(b"<< /Type /XObject /Subtype /Image /Width 2480 /Height 3508 /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /DCTDecode /Length %d >>\nstream\n" % len(j) + j + b"\nendstream")
    s = ("q 595 0 0 842 0 0 cm /Im0 Do Q BT 3 Tr /F1 12 Tf 72 700 Td (Pagina %d texto oculto del escaneo) Tj ET" % p).encode()
    c = add(b"<< /Length %d >>\nstream\n" % len(s) + s + b"\nendstream")
    data.append((im, c))
pages_id = len(objs) + P + 1
for im, c in data:
    page_ids.append(add(f"<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 595 842] /Resources << /XObject << /Im0 {im} 0 R >> /Font << /F1 {f1} 0 R >> >> /Contents {c} 0 R >>".encode()))
add(f"<< /Type /Pages /Kids [{' '.join(f'{k} 0 R' for k in page_ids)}] /Count {P} >>".encode())
cat = add(f"<< /Type /Catalog /Pages {pages_id} 0 R >>".encode())
out = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n"); offs = []
for i, body in enumerate(objs, 1):
    offs.append(len(out)); out += b"%d 0 obj\n" % i + body + b"\nendobj\n"
x = len(out); out += b"xref\n0 %d\n0000000000 65535 f \n" % (len(objs) + 1)
for o in offs: out += b"%010d 00000 n \n" % o
out += b"trailer\n<< /Size %d /Root %d 0 R >>\nstartxref\n%d\n%%%%EOF\n" % (len(objs) + 1, cat, x)
open("scan.pdf", "wb").write(out)
