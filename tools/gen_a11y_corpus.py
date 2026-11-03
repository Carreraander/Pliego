"""Genera tests/corpus/a11y/{bueno,malo}.pdf para probar el comprobador de accesibilidad.
bueno.pdf: etiquetado, idioma, título, encabezados H1>H2, figura con Alt, tabla con TH, campo con TU.
malo.pdf: sin etiquetar ni idioma ni título; con campo sin descripción y enlace sin descripción.
malo_etiquetado.pdf: etiquetado pero con figura sin Alt, salto H1>H3, tabla sin TH e idioma no válido.
Sin dependencias. Uso: python tools/gen_a11y_corpus.py"""
import pathlib


class Pdf:
    def __init__(self):
        self.objs = []

    def add(self, s):
        self.objs.append(s)
        return len(self.objs)

    def stream(self, d, data):
        return f"<< {d} /Length {len(data)} >>\nstream\n{data}\nendstream"

    def build(self, root, info=None):
        out = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
        offsets = []
        for i, body in enumerate(self.objs, 1):
            offsets.append(len(out))
            out += f"{i} 0 obj\n{body}\nendobj\n".encode("latin-1")
        xref = len(out)
        out += f"xref\n0 {len(self.objs)+1}\n0000000000 65535 f \n".encode()
        for off in offsets:
            out += f"{off:010d} 00000 n \n".encode()
        extra = f" /Info {info} 0 R" if info else ""
        out += f"trailer\n<< /Size {len(self.objs)+1} /Root {root} 0 R{extra} >>\nstartxref\n{xref}\n%%EOF\n".encode()
        return bytes(out)


def make(good):
    p = Pdf()
    font = p.add("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")
    content = p.add(p.stream("", "/H1 <</MCID 0>> BDC BT /F1 24 Tf 72 760 Td (Titulo) Tj ET EMC "
                                  + ("/H2 <</MCID 1>> BDC" if good else "/H3 <</MCID 1>> BDC")
                                  + " BT /F1 18 Tf 72 720 Td (Seccion) Tj ET EMC "
                                  "/Figure <</MCID 2>> BDC 100 600 50 50 re f EMC "
                                  "/TD <</MCID 3>> BDC BT /F1 12 Tf 72 560 Td (Celda) Tj ET EMC"))
    page_id = len(p.objs) + 1
    # Reservamos ids: page, pages, h1, h2, fig, cell, row, table, structroot, catalog...
    pages_id = page_id + 1
    h1_id, h2_id, fig_id, cell_id, row_id, table_id, sroot_id = (page_id + 2 + i for i in range(7))
    widget_id = sroot_id + 1
    p.add(f"<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 {font} 0 R >> >> "
          f"/Contents {content} 0 R /StructParents 0 /Annots [{widget_id} 0 R] "
          + ("/Tabs /S >>" if good else ">>"))
    p.add(f"<< /Type /Pages /Kids [{page_id} 0 R] /Count 1 >>")
    p.add(f"<< /Type /StructElem /S /H1 /P {sroot_id} 0 R /Pg {page_id} 0 R /K 0 >>")
    p.add(f"<< /Type /StructElem /S /{'H2' if good else 'H3'} /P {sroot_id} 0 R /Pg {page_id} 0 R /K 1 >>")
    p.add(f"<< /Type /StructElem /S /Figure /P {sroot_id} 0 R /Pg {page_id} 0 R /K 2"
          + (" /Alt (Un cuadrado negro) >>" if good else " >>"))
    p.add(f"<< /Type /StructElem /S /{'TH' if good else 'TD'} /Pg {page_id} 0 R /K 3 >>")
    p.add(f"<< /Type /StructElem /S /TR /Pg {page_id} 0 R /K [{cell_id} 0 R] >>")
    p.add(f"<< /Type /StructElem /S /Table /P {sroot_id} 0 R /Pg {page_id} 0 R /K [{row_id} 0 R] >>")
    p.add(f"<< /Type /StructTreeRoot /K [{h1_id} 0 R {h2_id} 0 R {fig_id} 0 R {table_id} 0 R] >>")
    p.add("<< /Type /Annot /Subtype /Widget /FT /Tx /T (nombre) /Rect [100 500 300 520] /DA (/Helv 12 Tf 0 g) "
          + ("/TU (Nombre completo) >>" if good else ">>"))
    info = p.add("<< /Title (Documento de prueba) >>") if good else None
    lang = "/Lang (es-ES)" if good else "/Lang (español)"
    catalog = p.add(f"<< /Type /Catalog /Pages {pages_id} 0 R /StructTreeRoot {sroot_id} 0 R "
                    f"/MarkInfo << /Marked true >> {lang} "
                    + ("/ViewerPreferences << /DisplayDocTitle true >> >>" if good else ">>"))
    return p.build(catalog, info)


def make_untagged():
    p = Pdf()
    font = p.add("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")
    content = p.add(p.stream("", "BT /F1 18 Tf 72 760 Td (Sin etiquetas) Tj ET"))
    page_id = len(p.objs) + 1
    pages_id = page_id + 1
    link_id = page_id + 2
    widget_id = link_id + 1
    p.add(f"<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 {font} 0 R >> >> "
          f"/Contents {content} 0 R /Annots [{link_id} 0 R {widget_id} 0 R] >>")
    p.add(f"<< /Type /Pages /Kids [{page_id} 0 R] /Count 1 >>")
    p.add("<< /Type /Annot /Subtype /Link /Rect [72 700 200 720] /A << /S /URI /URI (https://ejemplo.org) >> >>")
    p.add("<< /Type /Annot /Subtype /Widget /FT /Tx /T (campo) /Rect [100 500 300 520] /DA (/Helv 12 Tf 0 g) >>")
    catalog = p.add(f"<< /Type /Catalog /Pages {pages_id} 0 R >>")
    return p.build(catalog)


if __name__ == "__main__":
    out = pathlib.Path(__file__).resolve().parent.parent / "tests" / "corpus" / "a11y"
    out.mkdir(parents=True, exist_ok=True)
    (out / "bueno.pdf").write_bytes(make(True))
    (out / "malo_etiquetado.pdf").write_bytes(make(False))
    (out / "malo.pdf").write_bytes(make_untagged())
    print(out)
