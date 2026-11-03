"""Genera tests/corpus/protect/sucio.pdf: PDF con metadatos, XMP, JavaScript, acción de apertura,
adjunto incrustado, comentario, campo de formulario relleno y miniatura, para probar el saneado.
Sin dependencias. Uso: python tools/gen_protect_corpus.py"""
import pathlib

def build():
    objs = []
    def add(s):
        objs.append(s)
        return len(objs)
    def stream(d, data):
        return f"<< {d} /Length {len(data)} >>\nstream\n{data}\nendstream"
    font = add("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")
    content = add(stream("", "BT /F1 18 Tf 72 760 Td (Contenido confidencial de prueba) Tj ET q 1 0 0 1 72 500 cm /Fm1 Do Q"))
    thumb = add(stream("/Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8", "A"))
    js_action = add("<< /S /JavaScript /JS (app.alert\\('hola'\\);) >>")
    comment = add("<< /Type /Annot /Subtype /Text /Rect [100 700 120 720] /Contents (Comentario privado) /T (Alguien) >>")
    efile_data = add(stream("/Type /EmbeddedFile", "datos adjuntos secretos"))
    filespec = add(f"<< /Type /Filespec /F (secreto.txt) /UF (secreto.txt) /EF << /F {efile_data} 0 R >> >>")
    attach_annot = add(f"<< /Type /Annot /Subtype /FileAttachment /Rect [100 650 120 670] /FS {filespec} 0 R /Name /PushPin >>")
    field = add("<< /Type /Annot /Subtype /Widget /FT /Tx /T (nombre) /V (Valor del formulario) /Rect [100 600 300 620] "
                "/DA (/Helv 12 Tf 0 g) >>")
    ocg = add("<< /Type /OCG /Name (Capa oculta) >>")
    hidden_form = add(stream(f"/Type /XObject /Subtype /Form /BBox [0 0 300 50] /OC {ocg} 0 R /Resources << /Font << /F1 {font} 0 R >> >>",
                             "BT /F1 12 Tf 10 20 Td (Capa oculta secreta) Tj ET"))
    pages_id = len(objs) + 2
    page = add(f"<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 {font} 0 R >> /XObject << /Fm1 {hidden_form} 0 R >> >> "
               f"/Contents {content} 0 R /Thumb {thumb} 0 R /Annots [{comment} 0 R {attach_annot} 0 R {field} 0 R] "
               f"/AA << /O {js_action} 0 R >> >>")
    assert add(f"<< /Type /Pages /Kids [{page} 0 R] /Count 1 >>") == pages_id
    xmp_body = "<?xpacket begin='' id='W5M0MpCehiHzreSzNTczkc9d'?><x:xmpmeta xmlns:x='adobe:ns:meta/'>Autor secreto XMP</x:xmpmeta><?xpacket end='w'?>"
    xmp = add(stream("/Type /Metadata /Subtype /XML", xmp_body))
    info = add("<< /Title (Titulo privado) /Author (Autora Privada) /Producer (Generador) >>")
    names = add(f"<< /JavaScript << /Names [(init) {js_action} 0 R] >> /EmbeddedFiles << /Names [(secreto.txt) {filespec} 0 R] >> >>")
    acro = add(f"<< /Fields [{field} 0 R] /DA (/Helv 0 Tf 0 g) >>")
    catalog = add(f"<< /Type /Catalog /Pages {pages_id} 0 R /Metadata {xmp} 0 R /Names {names} 0 R "
                  f"/OpenAction {js_action} 0 R /AcroForm {acro} 0 R "
                  f"/OCProperties << /OCGs [{ocg} 0 R] /D << /OFF [{ocg} 0 R] >> >> >>")
    out = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
    offsets = []
    for i, body in enumerate(objs, 1):
        offsets.append(len(out))
        out += f"{i} 0 obj\n{body}\nendobj\n".encode("latin-1")
    xref = len(out)
    out += f"xref\n0 {len(objs)+1}\n0000000000 65535 f \n".encode()
    for off in offsets:
        out += f"{off:010d} 00000 n \n".encode()
    out += f"trailer\n<< /Size {len(objs)+1} /Root {catalog} 0 R /Info {info} 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode()
    return bytes(out)

if __name__ == "__main__":
    p = pathlib.Path(__file__).resolve().parent.parent / "tests" / "corpus" / "protect" / "sucio.pdf"
    p.write_bytes(build())
    print(p)
