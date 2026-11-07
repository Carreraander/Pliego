# Lecciones técnicas ya aprendidas

Trampas que costó descubrir. Consultar antes de tocar lo relacionado.

## mupdf-rs 0.8.0
- `Document::outlines()` hace `strlen(NULL)` (SEGV) con un marcador sin `/Title`. Se esquiva con un recorrido propio
  en `crates/pliego-engine-mupdf/src/outline.rs`. Borrador de issue: `borradores/w02-issue-mupdf-rs-outline-null-title.md`.
- `PdfDocument::open` solo acepta `&str` (usar `to_str()`). MuPDF mantiene el archivo abierto: en Windows bloquea el
  rename encima. En la CLI, abrir leyendo a memoria (`PdfDocument::from_bytes`) o `--in-place` falla con «Access denied».
- `max_align_t` al compilar mupdf-sys en Windows: falta `clang.exe` en el PATH para bindgen (`acr12-env.ps1` lo arregla).
  Arreglado aguas arriba (issues #222 y #258) pero sin publicar.
- No expone `pdf_subset_fonts` (borrador `borradores/w08-propuesta-mupdf-rs-subset-fuentes.md`), ni nombres ni render
  por plancha de `Separations` (solo `len/active_count`), ni linealización.
- No genera `/AP` para campos nuevos: `pliego-formdesign/src/ap.rs` los dibuja a mano.
- mujs: `this.getField()` devuelve null de forma inestable; `recalculate` evalúa `/CO` en nativo. Validaciones sí
  funcionan con `doc.enable_js()` + `set_value(.., false)`.

## Windows
- `sync_all` exige handle de escritura.
- `windows-sys` 0.61: el job object necesita `Win32_Security` y `Win32_System_Threading` además de `JobObjects`.
  Carrera pendiente: el hijo arranca antes de entrar al job (lanzar con `CREATE_SUSPENDED`).
- `tests/corpus/.gitattributes` fija `*.pdf -text`: el CRLF de autocrlf rompe la xref. Igual en `docs/handoff/borradores/`.
- `con.pdf` (y `nul`, `aux`, `prn`...) son nombres reservados.
- Excel COM rechaza rutas `\\?\C:\...` de `canonicalize`: quitar el prefijo.
- El worker se lanza sin consola (`CREATE_NO_WINDOW`, #100); antes dejaba conhost colgado.
- WebView2 por sí solo ocupa ~300 MB en reposo.

## Cargo y CI
- El `Cargo.lock` de `main` no siempre incluye los crates de `app/` (Tauri); cualquier `cargo` puede reescribirlo
  (~5000 líneas). Commitear solo la entrada del crate nuevo y `git checkout -- Cargo.lock` antes de `git add -A` si el
  diff es enorme y no es tuyo.
- Linux: excluir `pliego-app`/`pliego-app-service` (GTK). `pliego-cli` necesita `libtesseract-dev libleptonica-dev`.
- La CI usa un clippy más nuevo que el local.
- La CI de Windows en verde no prueba el entorno de acr12 si reutiliza la caché.

## Módulos
- `pliego-actions` no depende de los módulos: la CLI registra los pasos en `crates/pliego-cli/src/commands/run.rs`
  (`builtin_registry`); un paso nuevo es `r.register("modulo.op", typed(...))`. Diario `.pliego-run.json` para `--resume`.
- Firma: pyHanko 0.38 (en un venv fuera del repo) valida B-B/B-T/B-LT en local. Certificados de prueba generados en los
  tests, nunca reales en el repo.
- Ghostscript 10.02: `-dPDFX` es booleano; la versión PDF/X va en `PDFX_def.ps`. No se incluyen perfiles ICC.
- OCR en Windows: Tesseract 5.5 y Leptonica 1.87 de vcpkg (`x64-windows-release`); ver `docs/ocr.md`.

## PDFium (`pdfium-render`, W37)

- **No uses `Pdfium::default()`**: busca la biblioteca en el directorio actual y, si no, en el sistema (y entra en pánico si falla). Carga siempre con
  `Pdfium::bind_to_library(<ruta absoluta>)` (`pliego_pdf::pdfium_init`).
- **`FPDF_RenderPageBitmapWithMatrix` proyecta primero la página a un rectángulo de tamaño entero** (trunca ancho y alto): para que una matriz de MuPDF dé el mismo
  resultado hay que compensar `ancho/⌊ancho⌋` (véase `PageLink::render`). El modo con matriz no dibuja formularios (`FPDF_FFLDraw`): solo las apariencias `/AP`.
- **PDFium ignora `/UserUnit`** en el tamaño de página; MuPDF y Acrobat lo aplican.
- **Texto en el espacio de usuario**: origen abajo a la izquierda y sin girar; hay que convertirlo (`PageGeom`). Los caracteres «generados» son los espacios y saltos de línea que PDFium inventa.
- **Las cajas de búsqueda de PDFium son las del texto**, no las de línea de MuPDF: no sirven tal cual para cubrir con una redacción.
- **El texto de capas opcionales apagadas sí se extrae** con PDFium.
- `pdfium-render` con `thread_safe` serializa todas las llamadas con un cerrojo global (PDFium no admite hilos concurrentes); sin la característica `image_*` no arrastra la crate `image`.
- PDFium no soporta el filtro Brotli de PDF 2.0, y alguna página mal formada da `FormatError` donde MuPDF la repara: de ahí el respaldo a MuPDF del worker.


## PDF grandes (hueco P1 #11, W46)

- **lopdf guarda los flujos enteros en memoria**, además del `Vec<u8>` del archivo: un PDF de 1 GB ocupaba 2 GB al abrirse y, con la copia que
  PDFium pedía al dibujar, 3 GB; al buscar en todas las páginas subía a 4 GB (en Windows el job object del worker, 2 GB, lo mataba).
- **PDFium retiene los datos de las imágenes de cada página que carga hasta cerrar el documento** (3,5 MB por página en el escaneado de prueba).
  Por eso `pdfium::DocLink` recicla (cierra y abre) el documento leído de archivo tras `RECYCLE_AFTER_BYTES` leídos: abrirlo solo lee la tabla de
  referencias.
- `LoadOptions::filter` de lopdf es un `fn` sin estado y **solo mira si devuelve `Some`**: el objeto que queda es el que el filtro modificó en
  sitio (devolver otro objeto no sirve). Un flujo con contenido vacío se vuelve a leer del archivo salvo que se ponga `start_position = None`;
  los flujos `ObjStm`/`XRef` no se pueden vaciar (lopdf los usa para leer el resto).
- `FPDF_LoadCustomDocument` (`load_pdf_from_reader`): PDFium da por buena **cualquier lectura no vacía**, así que el lector debe llenar el búfer
  entero (salvo al final del archivo); y `m_FileLen` es `unsigned long` (32 bits en Windows): no se abren archivos de 4 GiB o más.
- **Sin manejador abierto entre lecturas** (`LazyFile`): en Windows un manejador abierto impide `rename` encima del archivo, que es como la app
  guarda las ediciones. El archivo puede cambiar bajo el visor; `LazyFile` lo detecta (tamaño y fecha) y falla limpio hasta que se recarga.
- **glibc (Linux) no devuelve a RssAnon la memoria de muchas asignaciones de unos MB** aunque se liberen (la medida parecía una fuga: 2 GB
  tras editar). Para medir en Linux: `MALLOC_MMAP_THRESHOLD_=1048576 MALLOC_TRIM_THRESHOLD_=1048576`. En Windows no ocurre.

## Edición de texto con emojis y fuentes sin glifos
- `pliego-edit` ya no falla si ninguna fuente tiene un carácter: `unicode::find_fonts` busca una sola fuente que lo cubra todo y, si no
  hay, combina varias (la que cubre más primero; símbolos y emojis de Windows `seguisym.ttf`/`seguiemj.ttf` al final). El texto se parte
  en trozos por fuente (`Stack::runs`) y cada trozo lleva su `Tf` y su subconjunto incrustado. Lo que ninguna fuente tiene sale como
  recuadro vacío y se avisa en el informe (`missing`).
- Los emojis salen en monocromo: el subconjunto (`subsetter`) conserva los contornos `glyf` pero no la capa de color COLR/CPAL.
- Las pruebas de `pliego-edit` necesitan PDFium: `PLIEGO_PDFIUM_DLL` con la biblioteca fijada en `tools/pdfium/pdfium.lock.json`.
- El formulario de Editar texto (`TextEditor.tsx`) se recoloca con `placeForm` para que no lo corte el borde de la página: debajo del
  texto, encima si no cabe, o pegado al borde inferior.
