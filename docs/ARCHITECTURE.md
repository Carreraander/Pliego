# Arquitectura de Pliego

Documento de diseño completo: https://claude.ai/artifact/Wu4ZXyGqToREgugipQuNgw

```
 App de escritorio (app/, Tauri 2 + SolidJS)   CLI (crates/pliego-cli)   Integración Windows (W10)
                    \                               |                        /
                     +------ Núcleo de servicios: documento, deshacer, trabajos, teselas ------+
                                                   |
   Módulos (crates/pliego-<modulo>): annot, forms, organize, sign, ocr, protect, redact,
                                     compare, optimize, convert, pdfa, edit, ...
                                                   |
   Capa PDF: pliego-pdf (única que habla con el motor PDF; hoy MuPDF, mañana PDFium + lopdf)
   Motores: MuPDF (en proceso aislado: pliego-worker), Tesseract, CNG/RustCrypto
```

## Crates base (ya existen)
| Crate | Contenido |
| --- | --- |
| `pliego-core` | `Error`/`Result`, `PageRange`, `JobContext`/`CancelToken`/`Progress`/`Report`, trait `PdfEngine` + `EngineDocument` |
| `pliego-pdf` | **Capa PDF** (W30): tipos propios (`Document`, `PdfDocument`, `Page`, `PdfPage`, `Object`, `Pixmap`, `TextPage`, anotaciones, campos, fuentes, imágenes, `WriteOptions`…) sobre el motor. Ver «Capa `pliego-pdf`» |
| `pliego-engine` | `DefaultEngine`/`DefaultDocument` (antes `pliego-engine-mupdf`): info, tamaño de página, render de página y tesela, texto, búsqueda, marcadores, capas, `encode_png`. Solo usa `pliego-pdf` |
| `pliego-cli` | Binario `pliego`: `info`, `render`, `text`, `search` |

## Contratos clave
- **Hilos:** los objetos de `pliego-pdf` (heredados de MuPDF) no son `Send`. Un documento abierto vive en un hilo propio; se le habla por canales.
- **Teselas:** la interfaz pide `pliego://tile/<doc>/<page>/<zoom>/<col>/<row>` (protocolo propio de Tauri);
  el backend devuelve PNG o WebP de `TileRequest.size` px (512 por defecto) y cachea por (doc, revisión, página, zoom, col, row).
- **Revisiones:** cada operación que modifica el documento sube su número de revisión; las teselas cacheadas de la
  revisión anterior se invalidan.
- **Proceso aislado:** `pliego-worker` ejecuta MuPDF para PDFs no confiables; si cae, la app reabre el documento
  y muestra un aviso, sin cerrarse.
- **Fuentes del sistema:** el motor se compila sin `system-fonts` (font-kit enumeraba todas las fuentes en cada
  búsqueda: ~1,3 s por documento en Windows). `pliego_pdf::init_fonts()` registra un `FontLoader` propio con
  un índice perezoso de `%WINDIR%\Fonts` y `%LOCALAPPDATA%\Microsoft\Windows\Fonts`; fuera de Windows no hace nada.
  Cada apertura de `pliego-pdf` (`Document::open`, `from_bytes`, `PdfDocument::new`…) lo llama sola; `main` de CLI y
  worker y `Index::add_folder` lo siguen llamando (es idempotente) por si hay trabajo previo a abrir.
- **Módulos:** biblioteca + subcomando CLI. La app los integra llamando a la biblioteca desde comandos Tauri.

## Capa `pliego-pdf` (contrato, congelado por W30)
Es el **único crate que depende del motor PDF** (`mupdf` hoy; PDFium + lopdf después, ola 5). El resto de crates
importa `pliego_pdf::*` y nunca `mupdf`. Reglas del contrato:
- **Tipos propios y públicos**: `Point`, `Rect`, `IRect`, `Quad`, `Matrix`, `Pixmap` (muestras contiguas, sin relleno),
  `Colorspace`, `Object`/`Value`, `TextPage`, `PdfAnnotation`, `PdfWidget`, `Font`, `Image`, `WriteOptions`, `Outline`…
  Ningún tipo de `mupdf` aparece en la API pública.
- **Forma de MuPDF a propósito**, para que migrar un crate sea mecánico: índices de página `i32` y
  `page_count() -> i32`, `get_dict`/`dict_put`/`array_push`, `as_name -> Vec<u8>`, iteradores como `std::vec::IntoIter`.
- **Errores**: siempre `pliego_core::Error` (`Error::Pdf(String)` para fallos del motor); no hay `map_err(pdf_err)`.
- **`Object` es una vista viva y clonable** del objeto del documento. No des por hecho que cambiar un objeto después
  de pasarlo a `dict_put`/`array_push` modifica el documento: vuelve a leerlo con `get_dict`/`get_array` y cambia esa
  vista. Así una implementación con lopdf (basada en rutas y copias) es posible. `Value` (enum de datos puros) y
  `object_from_value` / `Object::to_value` cubren los casos de «construir y pegar».
- **`PdfDocument` deriva a `Document`** (`Deref`); `Document::into_pdf()` convierte. Igual con `PdfPage` y `Page`.
- **Nada es `Send`**. Un documento vive en un hilo.
- **Solo se añade** a la API; lo que exista no cambia de forma (acuerdo de la ola 5). Las funciones nuevas llegan por
  PR pequeño al hilo de W30/gestión.
Estado de la API y de la migración de cada crate: `docs/handoff/flujos/w30.md`.

### Motor de render, texto y búsqueda (W37, W41)
El dibujo (`Page::to_pixmap`, `render_region`, `bounds`), el texto (`to_text_page`, `text`, `words`) y la búsqueda
(`Page::search`) van con **PDFium**; el modelo de objetos, el guardado, el cifrado, los enlaces, las anotaciones, los
formularios, las fuentes, las imágenes y la redacción, con **lopdf** (`pliego_pdf::native`). MuPDF ya no está (W41).
- **DLL:** `pdfium.dll` (`libpdfium.so`) se carga **solo por ruta absoluta**, nunca por `PATH` ni por el directorio
  actual: la de `PLIEGO_PDFIUM_DLL` o la que hay junto al ejecutable. Se carga sola la primera vez que se dibuja, se
  extrae texto o se busca; sin ella esas operaciones dan un error claro y todo lo demás funciona. El instalador la
  pone junto a la app, la CLI y el worker.
- **Copia en PDFium:** el documento se abre en PDFium desde el archivo original (o los bytes) la primera vez que se
  pide una página; si tiene cambios sin guardar en memoria (`State::generation`), PDFium lee una copia serializada sin
  cifrar y se rehace al cambiar la generación.
- **Coordenadas:** puntos, origen arriba a la izquierda, giro aplicado; PDFium trabaja en el espacio de usuario y la
  conversión está en `PageGeom` (`crates/pliego-pdf/src/pdfium.rs`).
- **Campos de formulario:** PDFium solo los dibuja con `FPDF_FFLDraw`, que ignora la matriz: con zoom y teselas (escala y
  traslación) se ajusta la página a su tamaño final y se desplaza el origen; con una matriz girada los campos no salen.
- **Sin CMYK:** PDFium dibuja en RGB. Las planchas de `pliego-print` reescriben los colores del contenido y dibujan una vez
  por tinta (véase `separations.rs`); las imágenes y degradados no se separan.
- **Sin JavaScript** (MuPDF/mujs era el único motor): los scripts de formularios necesitan un motor propio (flujo aparte).
- **`/UserUnit`:** ni el modelo propio ni PDFium lo aplican al tamaño de página (MuPDF lo multiplicaba).
- **Diferencias conocidas respecto a MuPDF:** `docs/handoff/flujos/w37.md`.

### `pliego_pdf::native` (W36): modelo de documento propio con lopdf
Reimplementa `Object`, `PdfDocument`, `PdfPage` y `GraftMap` con los mismos nombres y firmas que los de la raíz
de la capa (objetos, páginas, etiquetas, marcadores, adjuntos, capas, metadatos, cifrado, guardado completo e
incremental). No tiene render, texto ni búsqueda (PDFium, W37), anotaciones/widgets/fuentes/imágenes (W39)
ni conversión (W40); la redacción (W38) vive en `native::redact`. Desde W41 la raíz de `pliego-pdf` reexporta `native` y
`Document`/`Page` son envoltorios con PDFium encima.
Un `Object` nativo es una **ruta** (documento + objeto raíz + pasos), no un puntero: lo que `dict_put` inserta se
copia. Detalle, límites y pruebas en `docs/handoff/flujos/w36.md`.

### Redacción con el motor propio (W38): `pliego_pdf::native::PdfDocument::{apply_redactions, redact_areas}`
Reescribe los flujos de contenido de la página para que el texto, las imágenes y los vectores bajo las zonas **dejen de
existir** en el archivo (no solo de verse): sigue el estado gráfico y de texto, mide cada glifo con los anchos de la
fuente y lo quita de la cadena con ajustes de `TJ` para no mover el resto; copia y reescribe los formularios XObject
afectados; borra píxeles en una copia de la imagen (Flate/sin filtro/JPEG) o quita la imagen; quita vectores cubiertos;
elimina anotaciones bajo la zona, `/Thumb`, `/PieceInfo` y el `/ActualText` de lo marcado; y poda del documento lo que ya
no cuelga del trailer (y desactiva el guardado incremental). Regla de seguridad: lo que no se puede medir (fuentes sin
anchos, CMap predefinidos) se trata entero como sospechoso; nunca se adivina un ancho. En `pliego-redact` se elige con
`pliego_redact::apply` (único motor desde W41). Detalle y límites en
`docs/handoff/flujos/w38.md`.

W39 completa `native` por partes (estado y límites en `docs/handoff/flujos/w39.md`): **anotaciones** (`annot.rs`, con sus
apariencias en `appearance.rs`, escritas desde la especificación PDF y no desde MuPDF), **campos de formulario y aplanado**
(`widget.rs`, `formap.rs`, `flatten.rs`; sin JavaScript), **fuentes** (`font.rs`, las 14 estándar con Liberation/OFL en
`crates/pliego-pdf/fonts/liberation/`), **imágenes** (`image.rs`, solo PNG y JPEG al crearlas) y **recorrido del contenido** (`content.rs`, `image_placements`). Una anotación solo regenera su apariencia si algo la cambió (`State::annot_dirty`).

### Cifrado (W40, W41)
El cifrado de lectura (RC4-40, RC4-128, AES-128, AES-256 R5/R6; contraseñas de usuario y de propietario; permisos) y
de escritura (los mismos, con AES-256 R6 por defecto) vive en `pliego_pdf::native` y está probado con PDF de qpdf
(`crates/pliego-pdf/tests/fixtures/crypto/`). Desde W41 es el único motor: `PLIEGO_CRYPTO` y `CryptoBackend` ya no
existen. Un documento cifrado se abre con contraseña y PDFium lee la copia original con esa contraseña. Detalle en
`docs/handoff/flujos/w40.md`.

## Rendimiento objetivo (medido en acr12)
Arranque en frío < 1 s, abrir PDF de 500 páginas < 1,5 s, memoria en reposo < 150 MB, instalador < 40 MB.
