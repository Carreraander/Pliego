"""Genera tests/corpus/formdesign/flat-*.pdf (formularios planos sin campos) y flat-*.json (campos esperados).
Sin dependencias, sin datos personales. Uso: python tools/gen_formdesign_corpus.py"""
import json
import pathlib

OUT = pathlib.Path(__file__).resolve().parent.parent / "crates/pliego-formdesign/tests/corpus/formdesign"
W, H = 595, 842


class Page:
    def __init__(self):
        self.ops = []
        self.fields = []  # {"kind", "rect": [x0,y0,x1,y1] en espacio PDF}

    def text(self, x, y, s, size=10):
        s = s.replace("\\", "\\\\").replace("(", "\\(").replace(")", "\\)")
        self.ops.append(f"BT /F1 {size} Tf {x} {y} Td ({s}) Tj ET")

    def line(self, x0, y0, x1, y1, w=0.8):
        self.ops.append(f"{w} w {x0} {y0} m {x1} {y1} l S")

    def rect(self, x0, y0, x1, y1, w=0.8):
        self.ops.append(f"{w} w {x0} {y0} {x1 - x0} {y1 - y0} re S")

    def field(self, kind, x0, y0, x1, y1):
        self.fields.append({"kind": kind, "rect": [x0, y0, x1, y1]})

    # Atajos que dibujan el campo y registran lo esperado.
    def underline(self, x, y, w, label, size=10, colon=True):
        self.text(x, y + 2, label + (":" if colon else ""), size)
        lx = x + len(label) * size * 0.5 + 8
        self.line(lx, y, lx + w, y)
        self.field("text", lx, y, lx + w, y + 14)

    def box(self, x, y, w, h, label=None, inside=False, kind="text"):
        self.rect(x, y, x + w, y + h)
        if label:
            if inside:
                self.text(x + 3, y + h - 9, label, 7)
            else:
                self.text(x, y + h + 3, label, 8)
        self.field(kind, x, y, x + w, y + h)

    def check(self, x, y, label, size=11):
        self.rect(x, y, x + size, y + size)
        self.text(x + size + 5, y + 2, label, 10)
        self.field("checkbox", x, y, x + size, y + size)

    def comb(self, x, y, n, label, cell=16):
        self.text(x, y + cell + 3, label, 8)
        for i in range(n):
            self.rect(x + i * cell, y, x + (i + 1) * cell, y + cell)
        self.field("text", x, y, x + n * cell, y + cell)

    def build(self):
        stream = "\n".join(self.ops)
        objs = [
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            f"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {W} {H}] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>",
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
            f"<< /Length {len(stream)} >>\nstream\n{stream}\nendstream",
        ]
        out = bytearray(b"%PDF-1.7\n")
        offs = []
        for i, o in enumerate(objs, 1):
            offs.append(len(out))
            out += f"{i} 0 obj\n{o}\nendobj\n".encode("latin-1")
        x = len(out)
        out += f"xref\n0 {len(objs) + 1}\n0000000000 65535 f \n".encode()
        for o in offs:
            out += f"{o:010d} 00000 n \n".encode()
        out += f"trailer\n<< /Size {len(objs) + 1} /Root 1 0 R >>\nstartxref\n{x}\n%%EOF\n".encode()
        return bytes(out)


def f1():  # subrayados con etiqueta a la izquierda
    p = Page()
    p.text(50, 790, "SOLICITUD GENERICA", 16)
    for i, l in enumerate(["Nombre", "Apellidos", "Domicilio", "Localidad", "Provincia", "Telefono", "Correo", "Fecha"]):
        p.underline(50, 740 - i * 40, 300, l)
    return p


def f2():  # cajas con etiqueta encima
    p = Page()
    p.text(50, 790, "DATOS DE CONTACTO", 14)
    for i, l in enumerate(["Nombre", "Primer apellido", "Segundo apellido", "Ciudad"]):
        p.box(50, 700 - i * 60, 220, 22, l)
        p.box(310, 700 - i * 60, 220, 22, l + " (2)")
    return p


def f3():  # cuadricula con etiqueta dentro de cada celda
    p = Page()
    p.text(50, 790, "FICHA", 16)
    for r in range(5):
        for c in range(3):
            p.box(50 + c * 165, 700 - r * 36, 165, 36, f"Campo {r * 3 + c + 1}", inside=True)
    return p


def f4():  # casillas con etiqueta a la derecha
    p = Page()
    p.text(50, 790, "ENCUESTA", 16)
    for i, l in enumerate(["Acepto las condiciones", "Deseo recibir informacion", "Soy mayor de edad", "Resido en Espana",
                            "Tengo vehiculo", "Tengo mascota", "Trabajo por cuenta propia", "Otra situacion"]):
        p.check(60, 740 - i * 30, l)
    p.underline(60, 460, 280, "Observacion")
    p.underline(60, 420, 280, "Otra")
    return p


def f5():  # casillas de peine
    p = Page()
    p.text(50, 790, "IDENTIFICACION", 16)
    p.comb(50, 720, 9, "DNI / NIE")
    p.comb(50, 660, 5, "Codigo postal")
    p.comb(50, 600, 24, "IBAN", cell=15)
    p.comb(50, 540, 9, "Telefono")
    return p


def f6():  # areas multilinea
    p = Page()
    p.text(50, 790, "INCIDENCIA", 16)
    p.box(50, 640, 495, 110, "Descripcion")
    p.box(50, 480, 495, 120, "Observaciones")
    p.underline(50, 430, 300, "Firmado en")
    p.underline(50, 390, 300, "Fecha")
    return p


def f7():  # guiones bajos en el texto
    p = Page()
    p.text(50, 790, "DECLARACION", 16)
    for i, l in enumerate(["Nombre", "Cargo", "Empresa", "Direccion", "Telefono"]):
        y = 740 - i * 36
        p.text(50, y, f"{l}: " + "_" * 40, 11)
        x0 = 50 + (len(l) + 2) * 11 * 0.5 + 2
        p.field("text", x0, y - 2, x0 + 40 * 0.556 * 11, y + 12)
    return p


def f8():  # mixto administrativo
    p = Page()
    p.text(50, 800, "MODELO DE AUTORIZACION", 14)
    p.rect(40, 30, 555, 790)  # marco de la hoja
    p.text(50, 770, "1. DATOS DEL INTERESADO", 11)
    p.underline(50, 740, 250, "Nombre")
    p.underline(50, 710, 250, "Apellidos")
    p.comb(50, 660, 9, "NIF")
    p.box(300, 660, 230, 22, "Correo electronico")
    p.text(50, 620, "2. AUTORIZACIONES", 11)
    p.check(60, 590, "Autorizo la consulta")
    p.check(60, 565, "Autorizo la notificacion")
    p.check(300, 590, "No autorizo")
    p.text(50, 520, "3. EXPONE", 11)
    p.box(50, 400, 480, 100)
    p.underline(50, 360, 200, "Lugar")
    p.underline(300, 360, 200, "Fecha")
    return p


def f9():  # dos columnas, letra pequeña
    p = Page()
    p.text(50, 800, "REGISTRO", 14)
    for i in range(10):
        p.underline(40, 750 - i * 32, 150, f"Dato {i + 1}", size=8)
        p.underline(310, 750 - i * 32, 150, f"Valor {i + 1}", size=8)
    return p


FORMS = [f1, f2, f3, f4, f5, f6, f7, f8, f9]

if __name__ == "__main__":
    OUT.mkdir(parents=True, exist_ok=True)
    for i, f in enumerate(FORMS, 1):
        p = f()
        (OUT / f"flat-{i}.pdf").write_bytes(p.build())
        (OUT / f"flat-{i}.json").write_text(json.dumps({"fields": p.fields}, indent=1))
    print(f"{len(FORMS)} formularios en {OUT}")
