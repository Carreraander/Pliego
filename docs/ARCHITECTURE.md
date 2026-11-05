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

### `pliego_pdf::native` (W36): modelo de documento propio con lopdf
Reimplementa `Object`, `PdfDocument`, `PdfPage` y `GraftMap` con los mismos nombres y firmas que los de la raíz
de la capa (objetos, páginas, etiquetas, marcadores, adjuntos, capas, metadatos, cifrado, guardado completo e
incremental). No tiene render, texto ni búsqueda (PDFium, W37), anotaciones/widgets/fuentes/imágenes (W39),
redacción (W38) ni conversión (W40). Mientras convive con MuPDF, la raíz sigue exportando el motor MuPDF; el
cambio es sustituir esa exportación por `pub use native::*` cuando W37 aporte lo que falta, y W41 quita MuPDF.
Un `Object` nativo es una **ruta** (documento + objeto raíz + pasos), no un puntero: lo que `dict_put` inserta se
copia. Detalle, límites y pruebas en `docs/handoff/flujos/w36.md`.

## Rendimiento objetivo (medido en acr12)
Arranque en frío < 1 s, abrir PDF de 500 páginas < 1,5 s, memoria en reposo < 150 MB, instalador < 40 MB.
