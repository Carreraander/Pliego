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
| `viewer.present` Presentación | 🟡 | A: `present.test.ts`, manifiesto | V: pantalla completa |
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
| `forms.fill` campos y «Mi firma» | 🟡 | E2E: rellenar campo de texto con el documento abierto y comprobar el valor | W; V |
| `sign.digital` Firma digital | 🟡 | A: `sign.rs`; E2E2: firma con PFX de prueba desde un PDF abierto, la copia se abre en el visor y valida; no se puede firmar sobre el propio archivo | W: certificado del almacén de Windows (CNG), DNIe sin lector; V |
| `sign.validate` Validar firmas | 🟡 | A: `validate` tras firmar; E2E2: dos firmas del flujo | W |
| `sign.workflow` Flujo de firmas | 🟡 | A: `signflow_runs_in_order_and_keeps_an_audit_log`; E2E2: preparar, dos firmas en orden, auditoría de 3 entradas, cada paso se abre y se pinta | W, V |

### Organizar
| Herramienta | Estado | Evidencia | Falta |
| --- | --- | --- | --- |
| `pages.organize` (miniaturas, girar, borrar, reordenar, extraer) | 🟡 | A: `pages_copy` (reordenar, girar, borrar, duplicar) | V: arrastrar y soltar |
| `pages.merge` Combinar | 🟢 | A + E2E: combina dos PDF abiertos y cuenta páginas | — |
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
| `forms.design` Diseñar | 🟡 | E2E: añadir, cambiar y borrar un campo con el documento abierto | **W**; V |
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
| `compare.compare` Comparar | 🟡 | Conectado como panel de tarea (no pantalla): compara el documento abierto con otro y guarda `<nombre> (comparación).pdf`; A: `compare.rs` (informe, idénticos, peticiones inválidas) | V; pantalla lado a lado (diseño original) pendiente |
| `optimize.compress` Comprimir | 🟢 | A + E2E: copia con el mismo número de páginas | Documentos con contraseña (el panel no la guarda) |
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
| `batch.measure` Medir | 🟡 | A: `measure.rs`; E2E2: guardar escala y medida en el PDF abierto, repintar y deshacer | V |
| `batch.portfolio` Carteras | 🟢 | A + E2E: crear, listar y extraer | — |

### Asistente y actualizaciones
| Herramienta | Estado | Evidencia | Falta |
| --- | --- | --- | --- |
| `assistant.panel` descarga del modelo | 🟡 | A: `pliego-assistant` (huella y reanudación); TLS del sistema (#198) | **W**: repetir la descarga en acr12 |
| Actualizaciones (Ajustes) | 🟡 | A: `pliego-update` (ya usaba TLS del sistema) | W (ya probado en W22) |

## Documentos con contraseña (2026-11-05)

El panel no guardaba la contraseña: abrir un PDF protegido pedía la clave, pero Comprimir, Convertir, Sellos, Numerar,
Comprobaciones, Portafolio, Medir… la mandaban vacía y fallaban. Ahora la interfaz recuerda la contraseña **solo en
memoria** (`app/src/state/passwords.ts`, se olvida al cerrar el documento) y la añade a los trabajos y consultas sobre
ese mismo archivo (`runToolJob` y las consultas directas). Probado con vitest (`passwords.test.ts`), Playwright
(`app-smoke.spec.ts`: abrir con contraseña y comprimir manda `password`) y en Rust (`e2e_herramientas.rs`: comprobar,
analizar, comprimir, numerar y Bates sobre un PDF cifrado; las copias **siguen protegidas**).

Bug encontrado por esa prueba y arreglado: Comprimir un PDF cifrado con la contraseña correcta fallaba al final con
«contraseña necesaria» porque la comprobación del resultado no usaba la contraseña (`pliego-optimize`).

No cubierto todavía con contraseña: OCR (la CLI `pliego ocr` no tiene opción de contraseña), Comparar con el segundo
archivo cifrado, Combinar con entradas cifradas y las ediciones que escriben encima del propio PDF (comentar, rellenar,
editar texto, diseñar campos, medir).

## Cómo repetirlo

```
cargo test -p pliego-app-service --test e2e_documento_abierto --test e2e_herramientas   # Linux y Windows (acr12: por turno)
cd app && npx playwright test tests/visual/app-smoke.spec.ts --project=claro
cargo test -p pliego-engine -p pliego-core -p pliego-net
cargo test -p pliego-app-service --lib -- ocr:: compare::   # OCR con CLI simulada y Comparar
cd app && npm test
```

## Pendiente (siguientes PRs)

1. Repetir en acr12 las pruebas marcadas **W** y pasar a Ander las **V** (`gestion/revision-manual-ander.md`).
2. Contraseña en OCR, Comparar (archivo B), Combinar y las ediciones sobre el propio PDF (ver arriba).
3. Pantalla lado a lado de Comparar.
