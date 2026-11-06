# Huecos para la 1.0 (auditoría del 2026-11-06)

Revisión hecha a petición de Ander («¿no queda nada del desarrollo por hacer?») sobre `main` en `c65ec99`. Se compara
lo que hay con lo que espera quien llega de un editor PDF de pago y con lo que pide una versión pública para Windows.
Fuentes: `docs/handoff/`, `docs/qa/validacion-funciones.md`, los flujos, el código de la app (`app/src`,
`app/src-tauri`, `app/service`), los motores, el instalador y los issues/PRs abiertos (no hay issues; PRs abiertos:
#206 licencia, #227 memoria, #232 enlaces de W43).

**Fuera de esta lista porque ya tienen dueño:** traducción al inglés (hilo propio), edición tipo Acrobat (W43),
licencia (abogado), firma con DNIe (sin lector), certificado de firma de código (lo compra Ander). Lo que solo falta
**validar** en Windows o a ojo ya está en `docs/qa/validacion-funciones.md` (columnas W y V) y no se repite aquí.

Prioridades: **P0** bloquea la 1.0 (se nota en el primer minuto de uso); **P1** antes del lanzamiento público;
**P2** conviene en la 1.0 pero puede ir en una 1.0.x; **P3** después.

## P0 · Lo básico de un lector que no está

| # | Hueco | Qué se ve hoy | Evidencia | Propuesta |
| --- | --- | --- | --- | --- |
| 1 | **Imprimir** | No hay forma de imprimir un PDF. Ctrl+P lo coge WebView2 e imprime **la interfaz** de la app, no el documento | `file.print` solo está listado en `navigation/reference.ts`; ningún comando ni motor imprime en una impresora | Diálogo propio (impresora, páginas, copias, ajustar/tamaño real, orientación, escala de grises, comentarios sí/no) y envío con PDFium al DC de la impresora (`FPDF_RenderPage` sobre GDI) en el worker. Hilo nuevo |
| 2 | **Seleccionar y copiar texto** en Leer | Las páginas son imágenes (`<img alt="">`): no se puede seleccionar ni copiar nada fuera del modo Editar | `viewer/Viewer.tsx` solo pinta teselas; el worker solo da `PageText` (texto plano) y `SearchPage` | Capa de texto transparente por página (palabras con caja desde el worker), selección con ratón y Mayús+flechas, Ctrl+C, «Copiar» en el menú contextual. Sirve también al Narrador (P1 #9). Hilo nuevo |
| 3 | **Enlaces del PDF** en Leer | Los enlaces a otra página o a una web no hacen nada; los marcadores sí | El único que entiende enlaces es `tools/edit/LinkLayer.tsx` (modo Editar) | Capa de enlaces en Leer: salto interno con historial (Alt+←/→) y URL con confirmación antes de abrir el navegador. Con el #2 |
| 4 | **Atajos prometidos que no hacen nada** | La hoja de atajos (F1) lista Ctrl+O, Ctrl+W, Ctrl+Mayús+T, Ctrl+Tab, Ctrl+±/0/1/2, Ctrl+R, Ctrl+F, F3, Inicio/Fin/RePág/AvPág, Ctrl+D…, pero no tienen orden detrás | `registerReferenceShortcuts` solo los lista; nadie registra `run` | Registrar las órdenes del visor y de archivo. **Hecho en el PR 1 de este hilo** (`viewer/commands.ts`) |
| 5 | **Aceleradores y menú del navegador** | F5/Ctrl+R **recargan la app** (se cierran todos los documentos), Ctrl+P imprime la interfaz, Ctrl+F abre la búsqueda de Edge, Alt+← navega «atrás», y el clic derecho saca el menú de Edge (Atrás, Actualizar, Guardar como, Imprimir) | No hay `preventDefault` global ni se desactivan los aceleradores de WebView2 | Bloquear en la interfaz los atajos del navegador y su menú contextual fuera de los campos de texto. **Hecho en el PR 1 de este hilo** (`browserGuard.ts`) |
| 6 | **Un fallo interno deja un trabajo colgado** | Si una herramienta da un pánico, el hilo del trabajo muere y el centro de tareas lo muestra «en curso» para siempre | `pliego_appdata::jobs::JobRegistry::spawn` no recoge los pánicos | `catch_unwind` y terminar el trabajo con error y entrada en el log. **Hecho en el PR 1 de este hilo** |

## P1 · Antes del lanzamiento público

| # | Hueco | Evidencia | Propuesta |
| --- | --- | --- | --- |
| 7 | **Herramientas en un proceso aislado.** Los trabajos archivo → archivo corren en el proceso de la app con PDFium dentro: un PDF malicioso o roto que tumbe PDFium cierra la app entera (el visor sí está aislado en `pliego-worker`) | Decisión de la ola 4: «antes de la 1.0 pasan a un proceso aislado» (README); `app/service` enlaza `pliego-pdf` | Ejecutar los trabajos en un proceso hijo (`pliego.exe` o un modo del worker) con el mismo `JobId`, progreso y cancelación, como ya hace el OCR. Hilo nuevo |
| 8 | **Guardar como / Guardar una copia** (Ctrl+S, Ctrl+Mayús+S). Hoy cada herramienta guarda sola (encima con copia previa, o «(copia)» al lado), pero no se puede guardar el documento abierto en otra carpeta o con otro nombre | `file.save*` solo listados | «Guardar como» copia el archivo abierto (con su contraseña) a la ruta elegida y abre la copia; Ctrl+S confirma que ya está guardado. **Hecho en el PR 2 de este hilo** (`app/service/src/saveas.rs`, `viewer/commands.ts`) |
| 9 | **Accesibilidad del documento**: el Narrador no puede leer el contenido de las páginas (son imágenes) ni recorrerlas con el teclado | `Viewer.tsx` | Se resuelve con la capa de texto del #2 (roles y orden de lectura) y una prueba con Narrador en acr12. Más: alto contraste en el visor y foco visible en las páginas |
| 10 | **Funciones con motor pero sin pantalla** | `pliego-organize::write_outline`, `crop`, `pliego-a11y::autotag` y `FixOptions` (título, idioma, texto alternativo) solo están en la CLI; Propiedades y Adjuntos son de solo lectura | ~~Panel de Marcadores editable~~ (**hecho en este hilo**: añadir, renombrar, mover, sangrar y borrar sobre el PDF, con copia previa y deshacer), ~~«Recortar páginas» en Organizar~~ (**hecho en este hilo**: márgenes en mm, todas o algunas páginas, copia nueva), «Hacer accesible» (autoetiquetar + título/idioma/alt) en Accesibilidad, editar título/autor/asunto/palabras clave en Propiedades, añadir y quitar adjuntos. Un PR por herramienta, este hilo |
| 11 | **PDF grandes**: el worker y las herramientas cargan el PDF entero con lopdf además de los bytes y la copia de PDFium; deshacer serializa el documento completo en cada paso | `pliego-pdf/src/native/document.rs` (`load_mem`), `TODO(W02)` en `pliego-document/src/lib.rs`; los bancos usan PDF de 500 páginas pequeñas | Banco con PDF de 1 GB escaneado y de 5.000 páginas (abrir, desplazar, buscar, comentar, deshacer); límites de memoria del historial por tamaño; abrir solo con PDFium mientras no haga falta lopdf. Hilo nuevo (medir primero en acr12) |
| 12 | **Instalación para todo el equipo (UAC) de punta a punta** y **caché de iconos** | `--per-machine` existe pero nunca se probó entero (instalar, autoactualizar elevado, desinstalar); el icono genérico del buscador de Windows ya costó un día (caché) | Prueba en acr12/pc del recorrido por máquina; el instalador refresca la caché de iconos al instalar, actualizar y desinstalar (`SHChangeNotify` ya estaba; **`ie4uinit.exe -show` hecho en el PR 4 de este hilo**). Falta la prueba por máquina en acr12/pc (`flujos/w21.md`) |
| 13 | **Recuperación tras un cierre inesperado** | Inicio ya tiene el aviso «Pliego se cerró sin guardar N documentos» (`home/recovery.ts`), pero nadie llama a `setRecoverable` | Guardar la sesión (pestañas abiertas y página) y, si la app no cerró limpia, ofrecer reabrirlas. **Hecho en el PR 3 de este hilo** (`pliego_appdata::session`, `state/session.ts`). Falta: si muere WebView2 (`ProcessFailed`), recargar la interfaz y reabrir los documentos |
| 14 | **Informe de fallos local** | El log con rotación existe (#223), pero un cierre por fallo nativo no deja rastro y nadie se entera de que hubo un fallo | Marca de cierre limpio; al arrancar tras una caída, aviso con «Copiar informe» (versión, Windows y últimas líneas del log, que no llevan contenidos ni contraseñas y cambian la carpeta personal por `~`) y «Abrir carpeta de registros». **Marca y «Copiar informe» hechos en el PR 3 de este hilo.** Falta el volcado nativo opcional con WER `LocalDumps` para `pliego-*.exe`. Nada sale del equipo |
| 15 | **Publicar la 1.0** | La beta se compila a mano en acr12 (`tools/build-beta.ps1`); no hay releases en GitHub | Receta y script para la versión oficial desde una etiqueta: compilar, firmar el manifiesto, `SHA-256`, notas de versión y subida a GitHub Releases. Gestión, con Ander para las claves |

## P2 · Puede ir en una 1.0.x

| # | Hueco | Propuesta |
| --- | --- | --- |
| 16 | Escanear desde un escáner (WIA) a PDF con OCR | Botón «Escanear» en Inicio y en «Escanear y OCR» |
| 17 | Miniaturas y vista previa de PDF en el Explorador (`crates/pliego-shell`, pendiente desde W10) | Controlador de miniaturas y de vista previa con PDFium, opcional en el instalador |
| 18 | Firmar y validar PDF cifrados; acciones en lote con contraseña | `pliego-sign` rechaza `/Encrypt` (validacion-funciones.md) |
| 19 | «Hacer Pliego la app predeterminada para PDF» desde Ajustes (hoy solo lo ofrece el instalador) | Botón que abre `ms-settings:defaultapps?registeredAppUser=Pliego` |

## P3 · Después

Crear PDF desde el portapapeles o una captura, leer en voz alta, cifrado con certificado, varias ventanas.

## Reparto propuesto

- **Este hilo («Auditar y completar lo que falta para la 1.0»)**, PRs pequeños en este orden: #4-#6 (PR 1), #8, #12,
  #13-#14, #10 (una herramienta por PR), #19.
- **Hilos nuevos** (los abre gestión/coordinador): **Imprimir** (#1), **Texto seleccionable y enlaces en Leer** (#2, #3,
  #9), **Herramientas en proceso aislado** (#7), **PDF grandes** (#11). Opcionales para la 1.0.x: Escanear (#16) y
  Explorador (#17).
- **Gestión:** #15 (publicación) y repartir los turnos de acr12 de las pruebas nuevas.
