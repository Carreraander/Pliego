"""Genera tests/corpus/document/firmado.pdf: PDF de 1 página con un campo de firma (SigFlags=3) y un
diccionario de firma ficticio, para probar el guardado incremental. Sin dependencias.
Uso: python tools/gen_document_corpus.py"""
import pathlib


def build():
    objs = []

    def add(s):
        objs.append(s)
        return len(objs)

    stream = "BT /F1 24 Tf 72 760 Td (Documento firmado de prueba) Tj ET"
    font = add("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")
    content = add(f"<< /Length {len(stream)} >>\nstream\n{stream}\nendstream")
    pages_id = len(objs) + 2
    sigdict_id = len(objs) + 3
    field_id = len(objs) + 4
    page = add(f"<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 595 842] "
               f"/Resources << /Font << /F1 {font} 0 R >> >> /Contents {content} 0 R /Annots [{field_id} 0 R] >>")
    assert add(f"<< /Type /Pages /Kids [{page} 0 R] /Count 1 >>") == pages_id
    assert add("<< /Type /Sig /Filter /Adobe.PPKLite /SubFilter /adbe.pkcs7.detached "
               "/ByteRange [0 0 0 0] /Contents <00> /Reason (prueba) >>") == sigdict_id
    assert add(f"<< /Type /Annot /Subtype /Widget /FT /Sig /T (Firma1) /Rect [0 0 0 0] /P {page} 0 R "
               f"/V {sigdict_id} 0 R /F 132 >>") == field_id
    info = add("<< /Title (Firmado) >>")
    catalog = add(f"<< /Type /Catalog /Pages {pages_id} 0 R "
                  f"/AcroForm << /Fields [{field_id} 0 R] /SigFlags 3 >> >>")
    out = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
    offsets = []
    for i, body in enumerate(objs, 1):
        offsets.append(len(out))
        out += f"{i} 0 obj\n{body}\nendobj\n".encode("latin-1")
    xref = len(out)
    out += f"xref\n0 {len(objs)+1}\n0000000000 65535 f \n".encode()
    for off in offsets:
        out += f"{off:010d} 00000 n \n".encode()
    out += (f"trailer\n<< /Size {len(objs)+1} /Root {catalog} 0 R /Info {info} 0 R >>\n"
            f"startxref\n{xref}\n%%EOF\n").encode()
    return bytes(out)


if __name__ == "__main__":
    p = pathlib.Path(__file__).resolve().parent.parent / "tests" / "corpus" / "document" / "firmado.pdf"
    p.write_bytes(build())
    print(p)
