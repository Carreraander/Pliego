# PDF grandes: banco de memoria y rendimiento

Hueco P1 #11 de la auditoría del 2026-11-06 («PDF grandes»). Flujo [W46](../handoff/flujos/w46.md). Cómo medir, qué se exige y qué cifras hay.

## Qué se exige

Criterio de Ander (README del handoff): **150 MB de memoria privada entre app y worker** en reposo, y **500 MB** en el Administrador de tareas
contando WebView2. Con PDF grandes se aplica así:

| Caso | Exigencia |
| --- | --- |
| PDF de 1 GB (escaneado) abierto, desplazándose y buscando | app + worker **≤ 150 MB** en reposo, sin crecer con el tamaño del archivo. El pico al abrir (se lee el archivo una vez) es transitorio y queda por debajo del límite de 2 GB del worker |
| PDF de 5 000 páginas (texto) abierto, desplazándose y buscando en todo | app + worker **≤ 150 MB**; la búsqueda no hace crecer el worker |
| Comentar y deshacer en el de 1 GB | tras cada paso la app **no retiene copias del PDF** (reposo < 300 MB); los pasos de deshacer van a disco, no a memoria; el pico durante el guardado es transitorio |
| Nada de lo anterior debe tumbar el worker ni dejar un trabajo colgado | sin reinicios del worker durante el banco |

## Cómo se mide

Herramientas: `big-bench` (crate `pliego-qa`) genera los PDF y mide el worker real; la prueba manual `pdf_grande` mide la edición.

```
# Linux (lo que hizo el desarrollo; PDFium de tools/pdfium/pdfium.lock.json en $PLIEGO_PDFIUM_DLL)
cargo build --release -p pliego-worker -p pliego-qa --bin big-bench
big-bench gen-scan  /tmp/big/scan-1gb.pdf  --pages 300 --image-mb 3.5     # ~1,0 GB, 300 págs, JPEG de ruido + texto invisible
big-bench gen-text  /tmp/big/texto-5000.pdf --pages 5000                  # 23 MB, 5 000 págs, 500 marcadores
PLIEGO_WORKER=target/release/pliego-worker big-bench run /tmp/big/scan-1gb.pdf --search-pages 300
PLIEGO_BIG_PDF=/tmp/big/scan-1gb.pdf MALLOC_MMAP_THRESHOLD_=1048576 MALLOC_TRIM_THRESHOLD_=1048576 \
  cargo test --release -p pliego-app-service --test pdf_grande -- --ignored --nocapture

# acr12 / pc (Windows): todo junto, en D:, con turno
. .\tools\acr12-env.ps1
.\tools\acr12-turno.ps1 -Flujo w46 -- .\tools\bench-pdf-grandes.ps1
```

`big-bench run` pasa por `WorkerEngine` como la app: abrir, páginas e info, marcadores, primera tesela, 40 saltos repartidos por el documento,
última página, texto de una página y búsqueda en todas las páginas. Informa del tiempo y de la memoria **privada** de la app (`big-bench`
mismo) y del worker tras cada paso; en Linux, además, el pico del worker (`VmHWM`). El informe JSON va a `docs/qa/pdf-grandes/`.

Memoria privada = `RssAnon` en Linux y `PrivateMemorySize64` en Windows (lo que enseña el Administrador de tareas). La caché de teselas
(≤ 64 MB, en el proceso de la app) cuenta como memoria de la app: en el banco aparecen ~41 MB porque se piden 41 teselas.

> **Linux y glibc:** `glibc` no devuelve al sistema la memoria de muchas asignaciones de unos MB aunque se liberen; sin fijar
> `MALLOC_MMAP_THRESHOLD_` la prueba de edición «parece» retener 2 GB. En Windows el montón sí los devuelve; el criterio vale en Windows.

## Resultados

### Linux (contenedor de desarrollo, PDFium chromium/8138), 2026-11-06

Escaneado de 1 044 MB, 300 páginas (`PLIEGO_ENGINE=pdfium`):

| Paso | Antes: worker | Después: worker | App |
| --- | --- | --- | --- |
| Abrir | 2 091 MB | **6 MB** (pico transitorio 1 055 MB: se lee el archivo una vez) | 0,2 MB |
| Primera tesela | 3 140 MB | 13 MB | 1 MB |
| 40 saltos por el documento | 3 285 MB | 35 MB | 40 MB (caché de teselas) |
| Buscar «contrato» en las 300 páginas | **4 180 MB** | 29 MB | 41 MB |
| **Máximo app + worker** | **4 221 MB** | **80 MB** | |

Texto de 5 000 páginas (23 MB): app + worker de 160 MB a **74 MB** (con la apertura «solo para ver»; la búsqueda en las 5 000 páginas
sigue en ~7 s, 1,4 ms por página).

Edición del de 1 GB (`pdf_grande`, tres comentarios y tres «deshacer»; los comentarios en modo ligero):

| | Antes | Después |
| --- | --- | --- |
| Pasos de deshacer | 30 × 1 GB **en memoria** (tope del código, no del PDF: 30 GB) | en disco, junto al documento; 24 MB en memoria como mucho; 4 GB de disco como mucho |
| Memoria de la app tras editar | 1 GB retenido por recarga (`Source::Bytes`) | **3 MB** (el visor recarga leyendo el archivo; ya no pasa el PDF por la app) |
| Pico de la app durante un guardado (comentar) | 5,2 GB | **1,05 GB**, transitorio: es leer el archivo una vez al abrirlo en modo ligero; el guardado añade al final del archivo (incremental) y la app queda en 3 MB |
| Tiempo de un comentario sobre el de 1 GB | 7,3 s | 3,9-6,9 s (casi todo es copiar el archivo: copia de seguridad la primera vez, paso de deshacer y resultado) |

### acr12

Windows 10, acr12 (DESKTOP-41TJN0V), 2026-11-06, PDFium de `tools/pdfium/pdfium.lock.json`, worker real, `bench-pdf-grandes.ps1` con turno `w46`
sobre la cabeza de #241 (e85e5b2). JSON: `docs/qa/pdf-grandes/2026-11-06-1625-DESKTOP-41TJN0V-*.json`. Memoria privada (`PrivateMemorySize64`).

Escaneado de 1 044 MB, 300 páginas:

| Paso | Tiempo | App | Worker |
| --- | --- | --- | --- |
| Abrir | 1,98 s | 0,8 MB | 4,1 MB |
| Primera tesela | 284 ms | 1,8 MB | 10,7 MB |
| 40 saltos (el peor 95 ms) | 2,05 s | 41,1 MB | 25,2 MB |
| Última página | 48 ms | 42,1 MB | 28,5 MB |
| Texto de una página | 4 ms | 42,1 MB | 32,6 MB |
| Buscar «contrato» en 300 págs (300 coincidencias) | 2,19 s | 42,1 MB | 29,2 MB |
| **Máximo app + worker** | | **74,7 MB** (≤ 150 cumple) | |

Texto de 5 000 páginas (23 MB): abrir 121 ms; 40 saltos 238 ms; buscar en las 5 000 páginas (0 coincidencias) 9,1 s; **máximo app + worker 80,7 MB** (cumple).

Comentar y deshacer sobre el de 1 GB (`pdf_grande`, 399 s en total; el pico del worker en Windows no se mide: `worker_pico_mb` es nulo):

| Paso | Tiempo | App |
| --- | --- | --- |
| Abrir | 24,4 s | 1,0 MB (pico 6) |
| Comentar 1: guardar / recargar visor | 76,1 s / 16,7 s | 2,2 / 2,4 MB (pico 1 055) |
| Comentar 2: guardar / recargar visor | 40,9 s / 11,0 s | 1,8 / 1,8 MB |
| Comentar 3: guardar / recargar visor | 26,6 s / 9,0 s | 2,2 / 2,1 MB |
| Deshacer 1 / 2 / 3 | 35,8 / 36,2 / 38,0 s | 2,9 MB |

La app en reposo tras editar queda en 2 MB y tras deshacer en 3 MB: no crece con el archivo (criterio cumplido). Ojo con los **tiempos**: en
acr12 (disco D: y antivirus) un comentario sobre 1 GB tarda 27-76 s y cada «deshacer» 36-38 s, frente a 3,9-6,9 s en Linux; casi todo es copiar
el archivo de 1 GB (copia de seguridad, paso de deshacer y resultado). Para Ander puede ser lento; conviene vigilarlo.

Nota de la pasada: el script compilaba con `cargo build --release -p pliego-qa --bin big-bench -p pliego-worker`, que no construye `pliego-worker.exe`
(`--bin` filtra todos los paquetes) y `big-bench` fallaba con «no se pudo iniciar el proceso del motor»; ahora son dos compilaciones.

## Límites conocidos

- **Pico al abrir:** un PDF se lee entero una vez para construir el modelo; el búfer se suelta enseguida, pero el Administrador de tareas lo ve
  unos segundos (≈ tamaño del archivo). Por eso el máximo es 1,7 GB (`VIEW_ONLY_MAX_BYTES`, bajo el límite de 2 GB del worker); PDFium no abre
  archivos de 4 GiB o más en Windows.
- **Comentar en un PDF grande (≥ 64 MiB)** usa el modo ligero (`PdfDocument::open_lean` / `save_lean`): el modelo se carga sin los
  flujos grandes y los cambios se añaden al final del archivo. El pico es leer el archivo una vez (≈ su tamaño). Si el cambio tocara un
  flujo que no se cargó, o el PDF está cifrado o reparado, se usa el camino de siempre, que **reescribe el PDF entero** (lopdf): pico de
  ~3 GB durante segundos. Las demás herramientas que editan (formularios, texto, medir, páginas) siguen por ese camino.
- **Capas en PDF grandes:** mostrar u ocultar una capa necesita el documento entero; solo se admite hasta 256 MB (`LAYER_EDIT_MAX_BYTES`).
- El ejecutable `pliego` (CLI) abre los PDF enteros: sus pasos no usan el modo «solo para ver».
