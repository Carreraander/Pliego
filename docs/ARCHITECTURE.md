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
   Motores: pliego-engine-mupdf (MuPDF, en proceso aislado: pliego-worker), Tesseract, CNG/RustCrypto
```

## Crates base (ya existen)
| Crate | Contenido |
| --- | --- |
| `pliego-core` | `Error`/`Result`, `PageRange`, `JobContext`/`CancelToken`/`Progress`/`Report`, trait `PdfEngine` + `EngineDocument` |
| `pliego-engine-mupdf` | `MupdfEngine`/`MupdfDocument`: info, tamaño de página, render de página y tesela, texto, búsqueda, marcadores, `encode_png` |
| `pliego-cli` | Binario `pliego`: `info`, `render`, `text`, `search` |

## Contratos clave
- **Hilos:** los objetos de MuPDF no son `Send`. Un documento abierto vive en un hilo propio; se le habla por canales.
- **Teselas:** la interfaz pide `pliego://tile/<doc>/<page>/<zoom>/<col>/<row>` (protocolo propio de Tauri);
  el backend devuelve PNG o WebP de `TileRequest.size` px (512 por defecto) y cachea por (doc, revisión, página, zoom, col, row).
- **Revisiones:** cada operación que modifica el documento sube su número de revisión; las teselas cacheadas de la
  revisión anterior se invalidan.
- **Proceso aislado:** `pliego-worker` ejecuta MuPDF para PDFs no confiables; si cae, la app reabre el documento
  y muestra un aviso, sin cerrarse.
- **Módulos:** biblioteca + subcomando CLI. La app los integra llamando a la biblioteca desde comandos Tauri.

## Rendimiento objetivo (medido en acr12)
Arranque en frío < 1 s, abrir PDF de 500 páginas < 1,5 s, memoria en reposo < 150 MB, instalador < 40 MB.
