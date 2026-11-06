# Validación de funciones de la app (matriz)

Creada el 2026-11-05 tras la prueba de Ander en acr12 («no funciona ninguna funcionalidad»). Dice, herramienta por
herramienta (los `id` de `app/src/tools/base.ts`), **qué está comprobado, cómo y qué falta**. Se actualiza en cada PR.

## Qué se encontró (2026-11-05)

| Síntoma visto en acr12 | Causa | Arreglo |
| --- | --- | --- |
| Comentar, resaltar y «Añadir texto» sacan una y otra vez «error de E/S en Some("C:\\…\\x.pdf"): Access is denied. (os error 5)» | El worker abría el PDF **por ruta** y el motor de entonces (MuPDF) lo dejaba abierto; en Windows no se puede renombrar encima de un archivo abierto, y el guardado atómico de la app (temporal + renombrado) falla. Afectaba a todo lo que escribe encima del PDF abierto: comentar, rellenar, editar texto, diseñar formularios | El arreglo (abrir desde memoria, #198) ya lo da el motor nuevo: tras quitar MuPDF (#200), `pliego-pdf` lee todo el archivo al abrir y no deja ningún identificador abierto; la prueba `open_does_not_keep_the_file_open` lo fija |
| El mensaje salía con `Some(...)` y barras duplicadas, y se repetía | `Error::Io` formateaba `Option<PathBuf>` con `{:?}`; los avisos de error no se cierran solos y se apilaban | Mensaje legible con pista de «acceso denegado»; el mismo aviso ya no se apila |
| Asistente: «no se pudo descargar el modelo: io: invalid peer certificate: UnknownIssuer» | `ureq` con rustls y las raíces de Mozilla: no ve la CA de empresa/inspección TLS ni el almacén de Windows | Crate `pliego-net`: TLS del sistema (SChannel) y proxy de Windows para OCR, asistente y firma (#198) |
| Toda la interfaz en inglés (Read, Comment…) | El valor por defecto era «Automático» (el idioma de Windows, inglés en acr12) | Español por defecto (ajustes v2); «Automático» sigue disponible (#198) |
| «Reconocer texto (OCR)», «Enderezar y limpiar», «Idiomas de OCR» y «Comparar» siguen como «próximamente» | W24 dejó OCR y Comparar sin subir (el hilo se paró) | Conectados en el PR de OCR y Comparar (ver la matriz) |

Por qué no lo cazó nadie antes: las pruebas del servicio abrían el documento **y** guardaban en otro archivo, nunca
guardaban encima del documento abierto en el worker; en Linux además se puede renombrar encima de un archivo abierto,
así que la CI de Linux no lo distingue. La prueba nueva
[`app/service/tests/e2e_documento_abierto.rs`](../../app/service/tests/e2e_documento_abierto.rs) hace ese recorrido y
**debe ejecutarse en Windows** (acr12, por turno) para que tenga valor: `cargo test -p pliego-app-service --test
e2e_documento_abierto`.

## Leyenda

- **A** = automatizado y en verde en Linux (CI). **E2E** = `e2e_documento_abierto.rs` (documento abierto en el worker).
- **W** = falta repetirlo en Windows (acr12, turno de gestión): `cargo test -p pliego-app-service --test e2e_documento_abierto`
  y el resto de pruebas del servicio.
- **E2E2** = `e2e_herramientas.rs`: lo que genera cada herramienta se abre y se pinta en el worker.
- **V** = revisión visual de Ander (`gestion/revision-manual-ander.md`; el control de pantalla en acr12 se deniega solo).
- 🟢 comprobado de punta a punta (salvo lo marcado W/V) · 🟡 conectado y con pruebas, pero falta Windows/visual ·
  🔴 sin conectar a la app.

## Matriz

### Ver
| Herramienta | Estado | Evidencia | Falta |
| --- | --- | --- | --- |
| `viewer.present` Presentación | 🟡 | A: `present.test.ts`, manifiesto; Playwright `medir-presentacion.spec.ts` (F11 oculta el armazón, la página cabe entera, flechas/Inicio/Fin, Esc vuelve). **Bug hallado y arreglado (2026-11-05):** al ocultar el armazón la cuadrícula del shell dejaba el visor a 0 px de alto → F11 mostraba una pantalla en blanco | **Windows (pc, app real, 2026-11-05):** F11 a pantalla completa 1920×1080 sin barras, la página cabe, flechas/Inicio/Fin/Esc OK; vista pequeña pendiente de arreglar: con escala 125 % asomaban ~5 px de la página siguiente y se veía la barra de desplazamiento (margen y barra ajustados en el PR siguiente) | V |
| `viewer.layers` Capas | 🟢 | E2E2 (`e2e_herramientas.rs`): PDF con una capa, ocultar y mostrar cambia la tesela y no toca el archivo; capa inexistente da error | V (un PDF de verdad con capas) |
| `viewer.attachments` Adjuntos | 🟡 | A: `attachments.test.ts`; E2E: cartera (crear, listar, extraer) | V |
| `viewer.properties` Propiedades | 🟡 | A: `properties.test.ts`, `document_info` | V |
| Abrir, desplazarse, zoom, miniaturas, marcadores, búsqueda | 🟡 | A+E2E: abrir/teselas/búsqueda/esquema en `app/service/src/tests.rs`; ahora se abre desde memoria | W: abrir un PDF de 100+ páginas y guardar encima |

### Comentar
| Herramienta | Estado | Evidencia | Falta |
| --- | --- | --- | --- |
| `annotate.comment` resaltar, subrayar, nota, texto libre, dibujo | 🟡 | E2E: resaltar (por búsqueda), nota, texto libre, deshacer ×3 y copia de seguridad idéntica con el documento abierto; borrar | **W** (aquí estaba el error 5); V |
| `annotate.xfdf` Importar/exportar | 🟡 | A: `import_xfdf_copy`/`export_xfdf_copy`; E2E2: comentar con el documento abierto, exportar, importar en un PDF limpio y comprobar que se abre con el comentario | W, V |
| `annotate.summary` Resumen | 🟡 | A+E2E: `summary_of` tras anotar | V |
| `stamp.stamps` Sellos | 🟡 | A: `tests/stamps.rs`; E2E2: sello predefinido y de texto en todas las páginas, la copia se abre y la hoja cambia; texto vacío rechazado | V (Reader) |

### Rellenar y firmar
| Herramienta | Estado | Evidencia | Falta |
| --- | --- | --- | --- |
| `forms.fill` campos y «Mi firma» | 🟡 | E2E: rellenar campo de texto con el documento abierto y comprobar el valor; Playwright `rellenar-scripts.spec.ts` (backend simulado): campo con formato enseña `1.234,50 €` sin enfocar y el valor al editar, el filtro de tecleo rechaza letras y acepta cifras, `forms_fill` al confirmar y el botón con acción llama a `forms_run_action` y enseña el aviso del script | W; V |
| `sign.digital` Firma digital | 🟡 | A: `sign.rs`; E2E2: firma con PFX de prueba desde un PDF abierto, la copia se abre en el visor y valida; no se puede firmar sobre el propio archivo | **Windows (pc, 2026-11-05):** firma con certificado en una **tarjeta inteligente virtual TPM** (proveedor «Microsoft Smart Card Key Storage Provider», el mismo camino que el DNIe): `pliego sign --store` firma en 4–7 s, `verify --strict` válida, un bit alterado la rompe, cancelar el PIN da `Cancelled`. El PIN lo pide una ventana de Windows (`Credential Dialog Xaml Host`) que se puede automatizar con UI Automation (`PasswordField_0` + `OkButton`). Con PIN erróneo Windows reabre el cuadro y la firma espera (no da error): cuidar que la interfaz no se quede bloqueada. DNIe real: pendiente de lector; V |
| `sign.validate` Validar firmas | 🟡 | A: `validate` tras firmar; E2E2: dos firmas del flujo; vitest `problems.test.ts` y Playwright con la app en inglés: los avisos salen en el idioma de la app | W |
| `sign.workflow` Flujo de firmas | 🟡 | A: `signflow_runs_in_order_and_keeps_an_audit_log`; E2E2: preparar, dos firmas en orden, auditoría de 3 entradas, cada paso se abre y se pinta | W, V |

### Organizar
| Herramienta | Estado | Evidencia | Falta |
| --- | --- | --- | --- |
| `pages.organize` (miniaturas, girar, borrar, reordenar, extraer) | 🟡 | A: `pages_copy` (reordenar, girar, borrar, duplicar) | V: arrastrar y soltar |
| `pages.merge` Combinar | 🟢 | A + E2E: combina dos PDF abiertos y cuenta páginas; con un PDF cifrado distinto en cada archivo (`e2e_contrasena.rs`) | — |
| `pages.split` Dividir | 🟢 | A + E2E | — |
| `pages.insert` Insertar | 🟡 | A: `insert_copy` (en blanco y de otro PDF) | — |
| `pages.number` Numerar | 🟢 | A + E2E: el número «Página 1 de N» aparece en el texto de la hoja | V (Reader) |

### Editar y estampar
| Herramienta | Estado | Evidencia | Falta |
| --- | --- | --- | --- |
| `edit.content` editar texto / añadir texto | 🟡 | E2E: añadir y sustituir texto con el documento abierto, texto visible tras recargar | **W**; V (la recolocación tipo Acrobat no existe) |
| `edit.watermark` Marca de agua | 🟢 | A + E2E: «CONFIDENCIAL» aparece en el texto | V (Reader) |
| `edit.headerfooter`, `edit.background`, `edit.bates` | 🟢 | A: `tests/stamp.rs`; E2E2: cada copia se abre, la hoja cambia y el texto («ENCABEZADO», «CASO-000001») sale en la página | V (Reader) |

### Formularios
| Herramienta | Estado | Evidencia | Falta |
| --- | --- | --- | --- |
| `forms.design` Diseñar | 🟡 | E2E: añadir, cambiar y borrar un campo con el documento abierto; `pliego-formdesign/tests/render_forms.rs` (Linux con PDFium de la CI): los 15 campos de un formulario (texto, número con €, fecha, %, multilínea, contraseña, casillas, radios, combo, lista, botón, firma) se dibujan con tinta donde toca, en vivo y tras guardar. **Bugs hallados y arreglados (2026-11-05):** (1) con texto formateado por scripts el PDF llevaba `/NeedAppearances true` y PDFium/Poppler/Edge regeneraban la apariencia desde `/V`: se veía `1234.5` en vez de `1.234,50 €` y `0.255` en vez de `25,5%`; ahora se reconstruye la apariencia y se baja la marca; (2) los radios del diseñador se dibujaban con marco cuadrado | **W** (dibujo en la app instalada); V (Reader) |
| `forms.detect` Detectar campos | 🟡 | A: `tests/formdetect.rs` | — |

### OCR
| Herramienta | Estado | Evidencia | Falta |
| --- | --- | --- | --- |
| `ocr.recognize` Reconocer texto | 🟡 | Conectado (PR de OCR y Comparar): proceso hijo `pliego ocr` con progreso y cancelación; A: `ocr.rs` (CLI simulada: progreso, error, cancelar, idiomas); E2E con la CLI real y modelo `eng` (copia con texto) | **W**: acr12 (DLL de Tesseract junto a la app instalada); V |
| `ocr.clean` Enderezar y limpiar | 🟡 | Mismo trabajo con `--deskew --rotate --binarize --denoise`; las páginas conservan su aspecto (las opciones solo mejoran el reconocimiento) | W; V |
| `ocr.languages` Idiomas | 🟡 | `ocr_languages` (instalados y descargables) y descarga como trabajo con huella verificada; TLS del sistema | **W**: descarga de `spa` en acr12 |

### Proteger
| Herramienta | Estado | Evidencia | Falta |
| --- | --- | --- | --- |
| `protect.protect` Contraseña y permisos | 🟢 | A + E2E: cifra, el visor pide contraseña y con ella abre | V (Reader, aprobado en W07) |
| `protect.redact` Redactar | 🟢 | A + E2E: buscar correos, redactar y comprobar que el texto ya no se extrae | V (Reader) |
| `protect.sanitize` Sanear | 🟢 | A: `sanitize_deja_informe`; E2E2: la copia saneada se abre y se pinta con el mismo número de páginas | V |

### Convertir
| Herramienta | Estado | Evidencia | Falta |
| --- | --- | --- | --- |
| `convert.create` Crear PDF | 🟢 | A + E2E: de `.txt` a PDF y se abre en el visor | imágenes y HTML en W |
| `convert.export` imágenes/texto | 🟢 | A + E2E: PNG y TXT | — |
| `convert.office` Word/Excel/PowerPoint | 🟡 | A: `office_formats_save_a_copy` | W (Office real), V |
| `convert.pdfa` PDF/A | 🟡 | A: nivel desconocido rechazado, copia nueva; E2E2: la copia PDF/A-2b se abre y se pinta | W |

### Comparar y optimizar
| Herramienta | Estado | Evidencia | Falta |
| --- | --- | --- | --- |
| `compare.compare` Comparar | 🟡 | Panel de tarea: compara el documento abierto con otro y guarda `<nombre> (comparación).pdf`; A: `compare.rs` (informe, idénticos, peticiones inválidas). **Vista lado a lado** («Ver lado a lado», comando `compare_view`): dos columnas en un solo lienzo con un solo desplazamiento, páginas emparejadas (huecos en insertadas/eliminadas), cambios resaltados en los dos (quitado rojo, añadido verde, cambiado ámbar, formato punteado, aspecto con recuadro), F3/Mayús+F3 y contador; A: `compare_view` en `app/service`, `view.test.ts` (disposición, cambios, saltos) y e2e simulado `tests/visual/compare-view.spec.ts` (capturas en `test-results/compare-view/`) | **W**: comparar dos PDF reales con teselas de PDFium; V: aspecto y posición de los resaltados sobre páginas reales, legibilidad en oscuro y alto contraste |
| `optimize.compress` Comprimir | 🟢 | A + E2E: copia con el mismo número de páginas; con contraseña (E2E2) | — |
| `optimize.advanced` Optimizar avanzado | 🟡 | A: `analyze` | — |

### Accesibilidad, impresión, lote
| Herramienta | Estado | Evidencia | Falta |
| --- | --- | --- | --- |
| `access.check` | 🟢 | A + E2E: informe | V |
| `access.order` Orden de lectura | 🟡 | A: `reading_order`, `tab_order_copy`; E2E2: la copia con la tabulación ajustada se abre | V |
| `access.validate-pdfa` | 🟢 | A + E2E: validación PDF/A-2b | — |
| `print.preview` Vista previa de salida | 🟡 | A: `outputpreview.rs` | V |
| `print.preflight` Comprobación previa | 🟢 | A + E2E: perfil `basic` | — |
| `batch.actions` Acciones en lote | 🟡 | A: `tests/actions.rs`; E2E2: acción «borrar página 1» sobre un PDF abierto, salidas con una página menos | W |
| `batch.index` Índice de búsqueda | 🟢 | A + E2E: indexa una carpeta y encuentra «Pliego» | índice de 1.000 PDF en W |
| `batch.measure` Medir | 🟡 | A: `measure.rs`; E2E2: guardar escala y medida en el PDF abierto, repintar y deshacer; Playwright `medir-presentacion.spec.ts`: calibrar (200 px = 10 m), distancia (400 px → 20 m), área (100 m²) y `1:100`, con la geometría real del zoom y capturas | **Windows (pc, app real):** calibrar con 2 clics y 10 m → `1 pt = 0,09375 m`, distancia de 400 px = 20,00 m, área 200×200 px = 100,00 m² (perímetro 40,00 m); «Guardar medida» deja `line` y `polygon` en el PDF (`pliego annot list`) y la copia «(antes de medir)» idéntica | V |
| `batch.portfolio` Carteras | 🟢 | A + E2E: crear, listar y extraer | — |

### Asistente y actualizaciones
| Herramienta | Estado | Evidencia | Falta |
| --- | --- | --- | --- |
| `assistant.panel` descarga del modelo | 🟡 | A: `pliego-assistant` (huella y reanudación); TLS del sistema (#198) | **W**: repetir la descarga en acr12 |
| Actualizaciones (Ajustes) | 🟡 | A: `pliego-update` (ya usaba TLS del sistema) | W (ya probado en W22) |

## Documentos con contraseña (2026-11-05)

Regla: **la contraseña de un PDF abierto llega a todas las herramientas** y las copias que salen siguen protegidas con la
misma contraseña. La interfaz la recuerda **solo en memoria** (`app/src/state/passwords.ts`, se olvida al cerrar el
documento) y la añade a los trabajos y consultas sobre ese archivo (`runToolJob` y `withPassword` en las llamadas
directas). Para los archivos que una herramienta recibe **además** del abierto (Combinar, Comparar con otro PDF, Insertar
páginas de otro PDF) el comando `password_status` dice si piden contraseña y el panel la pide en un diálogo
(`state/unlock.ts`, `components/PasswordPrompt.tsx`) antes de lanzar el trabajo; esas contraseñas viven en el panel y no
se guardan en ninguna parte.

| Herramienta | Cómo llega la contraseña | Prueba |
| --- | --- | --- |
| Comprimir, Convertir, Sellos, Numerar, Bates, comprobaciones, Portafolio, Medir (consulta) | `password` del comando (PR #203) | `e2e_herramientas.rs`, `passwords.test.ts`, `app-smoke.spec.ts` |
| **OCR** y «Enderezar y limpiar» | `OcrRequest.password` → variable `PLIEGO_PASSWORD` del proceso hijo `pliego ocr` (también `--password`); nunca por la línea de órdenes | `e2e_contrasena.rs` (CLI real, copia protegida), `ocr.rs` (CLI simulada: entorno sí, argumentos no), `crates/pliego-cli/tests/ocr.rs` |
| **Comparar** con el segundo archivo cifrado | `passwordA` (la del abierto) y `passwordB` (se pide al elegir el archivo) | `e2e_contrasena.rs`, `app-smoke.spec.ts` (contraseña mala, correcta y cancelar) |
| **Combinar** | `MergeOptions.file_passwords` (una por archivo; `password` común como reserva) | `e2e_contrasena.rs`, `pliego-organize/tests/organize.rs`, `app-smoke.spec.ts` |
| **Insertar páginas** de otro PDF cifrado | `sourcePassword` (se pide al elegir el archivo) | `e2e_contrasena.rs`, `request.test.ts` |
| **Comentar**, rellenar, diseñar campos, editar y añadir texto, medir (guardar), XFDF, resumen de comentarios | `password` en `annot_*`, `forms_*`, `form_*`, `edit_text_*`, `measure_save`: el archivo se abre con ella, se edita y se **guarda con la misma protección**; el visor se recarga porque el worker conserva la contraseña | `e2e_contrasena.rs`: resaltar, borrar y deshacer; rellenar y diseñar; editar, añadir texto y medir; el original (la copia de seguridad) sigue cifrado |

Bugs encontrados al hacerlo (arreglados):
- Comprimir un PDF cifrado con la contraseña correcta fallaba al final por la comprobación del resultado (#203).
- El mensaje de error del OCR salía como la última línea de la traza si la CLI tenía `RUST_BACKTRACE`: ahora se usa la
  línea «Error: …» y la app quita esa variable del hijo.
- Las lecturas de campos y comentarios de un PDF cifrado devolvían una lista vacía sin avisar (no recibían la contraseña).

**Sin soporte todavía:** firmar o validar un PDF cifrado (`pliego-sign` lo rechaza al ver `/Encrypt`; `sign_validate` y
el flujo de firmas no reciben contraseña) y las acciones en lote sobre carpetas (cada archivo tendría la suya).

Falta en Windows (acr12 / «pc»): repetir `cargo test -p pliego-app-service --test e2e_contrasena` con `PLIEGO_CLI`
apuntando a la CLI compilada, y la comprobación visual de Ander (abrir un PDF cifrado, comentar, comprimir y combinar
con otro cifrado).

## Idioma de las firmas (2026-11-05)

Con la app en inglés, la apariencia visible de la firma («Firmado digitalmente por…», «Fecha», «Motivo», «Lugar»), el
nombre del campo («FirmaN»), la hoja y el motivo del flujo de firmas y los avisos de «Validar firmas» seguían en
español (los textos salían del motor). Ahora `SignOptions.lang` (`es`/`en`, por defecto `es`) decide el texto que la
firma deja en el PDF y la interfaz manda el idioma de la app; el motor da una clave estable por aviso
(`problem_keys`) y la interfaz los traduce. Los sufijos de las copias («firmado», «para firmar») también salen de la
interfaz. Sin cubrir: el error libre al leer una firma corrupta (`other`).

## Centro de tareas en inglés (2026-11-05)

Los títulos, el progreso («Guardando…») y los resúmenes («3 páginas numeradas») de los trabajos salen de los motores
en español (la CLI usa los mismos). La interfaz los traduce en `app/src/jobs/translate.ts` con una tabla de plantillas
(`RULES`, es → en); un texto sin regla se muestra tal cual. `translate.test.ts` recorre los fuentes de Rust y falla si
aparece una plantilla sin regla, así que añadir un mensaje nuevo obliga a traducirlo. Sin cubrir: los errores libres
(`job.error`), que siguen en español.

## Registro de diagnóstico (2026-11-06)

Un fallo en el equipo de una persona no dejaba rastro. Ahora hay un log con rotación (`pliego_core::log`; ver
`docs/app-integration.md`): arranque, worker, documentos que no abren, trabajos fallidos, pánicos y actualizaciones, sin
contenidos ni contraseñas. Pruebas: unitarias de rotación y saneado, `crates/pliego-appdata/tests/log_trabajos.rs` y el
botón de Ajustes en `app-smoke.spec.ts`. **W** pendiente en Windows: arrancar la app, provocar un fallo (abrir un PDF
dañado), comprobar `%APPDATA%\Pliego\logs\pliego.log` y que «Abrir carpeta de registros» abre el Explorador.

## Atajos del visor y del navegador (2026-11-06)

La hoja de atajos (F1) prometía órdenes que no tenía nadie y WebView2 dejaba pasar los atajos de Edge (F5/Ctrl+R
recargaban la app y cerraban los documentos; Ctrl+P imprimía la ventana; clic derecho con «Actualizar» o «Imprimir»).
Ahora `app/src/viewer/commands.ts` registra Ctrl+O, Ctrl+W, Ctrl+Mayús+T, Ctrl+Tab/Ctrl+Mayús+Tab, Inicio/Fin/RePág/AvPág
(solo con el foco en el visor de Leer), Ctrl+±/0/1/2 y Ctrl+rueda, Ctrl+R (girar la vista), Ctrl+F y F3/Mayús+F3;
Ctrl+P avisa de que la impresión llega más adelante (`docs/handoff/huecos-1.0.md`, P0 #1). `app/src/browserGuard.ts` para
lo que el navegador haría solo y su menú contextual (salvo en los campos de texto; F12 y Ctrl+Mayús+I solo en
desarrollo). Pruebas: `browserGuard.test.ts`, `viewer/commands.test.ts` y tres casos de `app-smoke.spec.ts`. **W**: en la
app instalada, F5, Ctrl+R, Ctrl+P, Alt+← y el clic derecho no deben recargar, imprimir ni sacar el menú de Edge.

**Guardar como** (Ctrl+Mayús+S y menú de la app): copia el archivo abierto a la ruta elegida (temporal y renombrado,
`app/service/src/saveas.rs`), abre la copia con la misma contraseña y cierra la pestaña anterior. Ctrl+S solo avisa de
que los cambios ya están en el archivo (las herramientas guardan al aplicar). Pruebas: `saveas::tests` y
`app-smoke.spec.ts`. **W**: guardar como sobre un PDF cifrado y en otra unidad.

Los trabajos en segundo plano ya no se quedan «en curso» si la herramienta da un pánico: `JobRegistry::spawn` lo recoge y
termina el trabajo con el error «fallo interno de la herramienta» (prueba `panic_finishes_the_job_with_an_error`).

## Cómo repetirlo

```
cargo test -p pliego-app-service --test e2e_documento_abierto --test e2e_herramientas --test e2e_contrasena   # Linux y Windows (acr12: por turno; con PLIEGO_PDFIUM_DLL y, para el OCR, PLIEGO_CLI)
cd app && npx playwright test tests/visual/app-smoke.spec.ts --project=claro
cargo test -p pliego-engine -p pliego-core -p pliego-net
cargo test -p pliego-app-service --lib -- ocr:: compare::   # OCR con CLI simulada y Comparar
cd app && npm test
```

## Pendiente (siguientes PRs)

1. Repetir en acr12 las pruebas marcadas **W** y pasar a Ander las **V** (`gestion/revision-manual-ander.md`).
2. Repetir en Windows las pruebas con contraseña (`e2e_contrasena.rs`).
3. ~~Pantalla lado a lado de Comparar~~ (hecha; falta Windows y revisión visual).
