# Traspaso del proyecto Pliego (2026-11-04)

Este directorio recoge **todo el estado del proyecto** que hasta ahora vivía fuera del repo (hilos de Claude, memoria
del proyecto y carpeta compartida). El proyecto se traslada a otra cuenta de Claude; allí solo seguirán existiendo
este repositorio (`Carreraander/Pliego`) y el equipo de pruebas **acr12**. Quien retome el trabajo debe poder
hacerlo leyendo solo este directorio, `CLAUDE.md`, `docs/CONVENTIONS.md`, `docs/ARCHITECTURE.md`,
`docs/workstreams/` y `docs/design/`.

| Archivo | Qué contiene |
| --- | --- |
| `README.md` (este) | Estado global, PRs abiertos, decisiones y tareas de Ander |
| [`reglas-operativas.md`](reglas-operativas.md) | Organización de hilos, modelos, autonomía, fusión de PRs, ramas y acr12 |
| [`memoria.md`](memoria.md) | Hechos para reconstruir la memoria del proyecto nuevo, uno por archivo |
| [`lecciones-tecnicas.md`](lecciones-tecnicas.md) | Trampas ya descubiertas (Windows, mupdf-rs, Cargo.lock, CI) |
| [`flujos/`](flujos/) | Un archivo por flujo W01-W22: estado, PRs, qué falta para cerrarlo |
| [`prompt-nuevo-proyecto.md`](prompt-nuevo-proyecto.md) | Prompt para el coordinador del proyecto nuevo y pasos manuales de Ander |
| [`borradores/`](borradores/) | Borradores sin publicar (issues para mupdf-rs) que estaban en la carpeta compartida |

Los prompts de cada flujo (`docs/workstreams/wNN-*.md`) y el diseño de la app (`docs/design/`, con prototipo y
tokens) ya estaban en el repo; la copia de la carpeta compartida era igual o más antigua, así que **la versión del
repo es la buena**.

## Qué es Pliego

Alternativa libre (AGPL-3.0-or-later) y ligera a Adobe Acrobat Pro para Windows: núcleo Rust sobre MuPDF, interfaz
Tauri 2 + WebView2 + SolidJS, CLI `pliego`, OCR con Tesseract, firma PAdES con RustCrypto + CNG (DNIe/FNMT),
asistente IA local opcional (llama.cpp). Instalador propio (`Pliego Setup`) y autoactualizaciones firmadas Ed25519.
Idioma del proyecto: español (identificadores en inglés). Sin marca, iconos ni disposición de Adobe.

Documentos de diseño originales (artefactos de la cuenta antigua; pueden no abrirse desde la nueva):
diseño general https://claude.ai/artifact/Wu4ZXyGqToREgugipQuNgw, prototipo navegable de la app
https://claude.ai/artifact/Pin9i2YGZRQUuDdft48FVR. El contenido equivalente está en `docs/design/`
(`prototipo.html` se abre en local).

## Estado global a 2026-11-04 14:05 UTC

- **Proyecto trasladado a la cuenta nueva** (2026-11-04). Hilo de gestión: «Gestionar el desarrollo de Pliego»; un
  hilo Sonnet por flujo. La cola de acr12 (una sesión de Remote Control a la vez) la lleva gestión.
- **Fusionados tras el traspaso:** #104 (W03), #106 (W05 almacén de Windows), #107 (W03 docs), #93 (W10 benchmarks
  con cifras de acr12), #110 (W21), #111 (W02 job object con `CREATE_SUSPENDED`), #112 (W01 worker como sidecar y
  `forget`), #113 (W06 catálogo de modelos), #114 (W07 docs), #115 (W16 comparación con Ghostscript), #116 (W22
  servicio de actualizaciones), #118 (W05 OCSP, VRI, B-LTA), #119 (W19), #121 (W12 PDF/A: `/ID`, capas, adjuntos y
  `/AA` de página; sustituye a #96, cerrado), #120 (W21: `--update` elevado verifica la firma Ed25519 del manifiesto;
  marca `install-kind`), #122 (W22: interfaz de actualizaciones, reinicio por equipo con `--manifest`, VPS compartido
  con `sites.d`), #123 y #125 (web de pdfpliego.com con demo; **publicada** en el VPS), #124 (W15: MuPDF sin
  `system-fonts`, `FontLoader` propio), #126 (W03 docs), #127 (W21 `tools/build-beta.ps1`), #128 y #129 (W22: servidor
  de actualizaciones real en actualizaciones.pdfpliego.com, claves públicas de producción), #130 y #131 (W10: lab «pc»).
- **Beta:** `Pliego-Setup-0.1.0-beta.exe` (16,6 MB) en acr12, `D:\dev\pliego-beta\`. Sin firma de código todavía (Ander
  comprará el certificado). E2E de autoactualización en curso (W22).
- **Claves:** firma Ed25519 de producción solo en acr12 (`D:\dev-cache\release-keys`) y en el USB de Ander; nunca en
  GitHub, el VPS ni el repo (solo las públicas). SSH de despliegue en «pc» y en el secreto `UPDATE_SSH_KEY`.
- **Equipos de pruebas:** acr12 (cola principal) y «pc» (Windows 11, solo tareas ligeras: C: casi lleno).
  Computer use desde Remote Control se deniega solo (W03, 3 intentos): falta decidir con Ander cómo revisar en Reader/Edge.
- **Hueco detectado (2026-11-04, tarde):** las funciones existen como crates y CLI, pero en la app solo funcionan Leer,
  la búsqueda y el asistente; el resto sale como «próximamente» porque ningún flujo tenía asignado conectarlas. Se abre la
  **ola 4 (W23-W29)**: [`docs/workstreams/ola4-conectar-herramientas.md`](../workstreams/ola4-conectar-herramientas.md).
- **Decisión de motor y licencia (Ander, 2026-11-04 23:22):** opción B1 del estudio [`licencia-y-motor.md`](../design/licencia-y-motor.md): sustituir MuPDF por PDFium + lopdf tras una capa propia `pliego-pdf`; licencia candidata BSL 1.1 con uso gratuito para empresas (paga quien venda, integre en un producto de pago u ofrezca Pliego como servicio de pago). Plan: **ola 5 (W30-W41)**, [`docs/workstreams/ola5-motor-propio.md`](../workstreams/ola5-motor-propio.md). Esto **sustituye** a «AGPL + MuPDF» (2026-11-02) cuando se complete; hasta entonces `LICENSE`/`deny.toml` no cambian y falta la revisión legal.
- **PRs abiertos:** ninguno de código.
- **Issues abiertas:** ninguna.

### Resumen por flujo

| Flujo | Tema | Estado | Qué falta para cerrarlo |
| --- | --- | --- | --- |
| [W01](flujos/w01.md) | App y visor | Fusionado | Medir los ~1,9 s de la primera página en acr12, locale `es` por defecto, worker como sidecar, criterio de memoria |
| [W02](flujos/w02.md) | Motor, worker, guardado | Fusionado | Cifras de acr12 (tesela 512 px, job object, PDFs patológicos), carrera del job object, `outline` en fuzz.sh |
| [W03](flujos/w03.md) | Anotaciones y formularios | Fusionado | Capturas Reader/Edge (necesita el permiso de pantalla) |
| [W04](flujos/w04.md) | Organizar y sellos | Fusionado | Combinar 50 PDFs en < 5 s en acr12, captura de Bates/marca de agua en Reader |
| [W05](flujos/w05.md) | Firma PAdES | Fusionado (parcial) | OCSP, B-LTA, VRI, captura en Reader, DNIe (acr12 no tiene lector) |
| [W06](flujos/w06.md) | OCR | Fusionado | Catálogo de modelos con SHA-256, CER y benchmark de 20 páginas en acr12 |
| [W07](flujos/w07.md) | Proteger y redactar | Fusionado | Prueba en Acrobat Reader (contraseña, permisos, saneado) |
| [W08](flujos/w08.md) | Comparar y optimizar | Fusionado | Medir en acr12, abrir resultados en Reader; aplazados linealizar, subset de fuentes, JBIG2/CCITT |
| [W09](flujos/w09.md) | Crear y convertir | Fusionado | Prueba con Word, Excel y PowerPoint reales en acr12 |
| [W10](flujos/w10.md) | Build, QA, Windows | Fusionado (parcial) + **PR #93** | Benchmarks, `crates/pliego-shell`, MSI/ZIP, firma de código |
| [W11](flujos/w11.md) | Edición de texto e imágenes | Fusionado | **Ander:** abrir en Reader los PDFs de `D:\dev\pliego-w11-out\` |
| [W12](flujos/w12.md) | PDF/A, PDF/UA | **Cerrado** (44/44 veraPDF) | Nada |
| [W13](flujos/w13.md) | Diseñador de formularios | Fusionado | Prueba en Reader (10+5 → «15,00 €», 200 rechazado); ONNX opcional no hecho |
| [W14](flujos/w14.md) | Exportar a Office | Fusionado | Revisión visual con Word real en acr12; unificar extracción con pliego-convert |
| [W15](flujos/w15.md) | Lote, índice, medidas | Fusionado (#124) | Repetir en acr12 el índice de 1.000 PDFs y el render sin fuentes incrustadas con el `FontLoader` propio |
| [W16](flujos/w16.md) | Impresión | Fusionado | Prueba en acr12; planchas de color directo y sobreimpresión (limitación de mupdf 0.8) |
| [W17](flujos/w17.md) | Flujo de firmas | Fusionado | Tests de signflow en acr12 con corpus limpio, tres firmantes en Reader |
| [W18](flujos/w18.md) | Asistente IA local | Fusionado | Compilar CLI/app con `--features llama` en Windows y probar de punta a punta; `Session::forget` |
| [W19](flujos/w19.md) | Armazón de la interfaz | Fusionado | Arranque en frío < 1 s, ventana, F6/F4/F9 en acr12; ocultar barra de modos en Inicio/Ajustes |
| [W20](flujos/w20.md) | Navegación, Inicio, Ajustes | Fusionado | Teclado y Narrador en acr12 (**Ander:** aceptar el permiso de pantalla) |
| [W21](flujos/w21.md) | Instalador y lanzador | Fusionado | Mica, escalas 100/150/200 %, Narrador, sin WebView2, por máquina (UAC) |
| [W22](flujos/w22.md) | Autoactualizaciones | Fusionado (parcial) | Comandos Tauri, directiva HKLM, UI en Ajustes y prueba de extremo a extremo en acr12 |
| [W25](flujos/w25.md) | Conectar Proteger y Redactar | Fusionado | Falta prueba en acr12 y revisión visual (revision-manual-ander.md) |
| [W42](flujos/w42.md) | Motor de JavaScript de formularios (Boa) | En curso | Conectarlo a `pliego_pdf::native` y a la app |
| [W43](flujos/w43.md) | Edición tipo Acrobat (recolocar texto; imágenes, vectores y enlaces) | En curso | PR 1: motor de movimiento y recolocación; falta la interfaz y validar en acr12/pc |

## Decisiones de gestión de la ola 4 (2026-11-04)

- Los trabajos archivo → archivo corren en el proceso de la app (patrón de `docs/app-integration.md`); antes de la 1.0
  pasan a un proceso aislado con el mismo `JobId` y eventos (pendiente de W02).
- OCR desde la app: `pliego.exe ocr` como proceso hijo (la app no enlaza Tesseract), con progreso JSON por líneas.
- Patrón común: lógica en `app/service/src/<área>.rs` con pruebas; comandos Tauri que solo traducen; copias con
  `pliego_appdata::copyjob`; frontend con `runToolJob` y una línea en `registerAppTools()`.
- «Numerar páginas» imprime el número en la hoja (`pliego_stamp::header_footer`); las etiquetas del PDF son opción avanzada.

## Decisiones tomadas (no reabrir sin Ander)

- **Licencia AGPL-3.0-or-later con MuPDF** (Ander, 2026-11-02; **a sustituir por B1 + licencia nueva al terminar la ola 5**, ver arriba), sabiendo que una edición cerrada o comercial
  exigiría licencia de Artifex. La alternativa descartada era Apache-2.0 + PDFium + qpdf.
- **Organización:** un hilo de gestión con **Opus 5.5** y un hilo de implementación con **Sonnet 5.5** por flujo;
  el diseño de la app lo lleva un hilo Opus aparte (ver `reglas-operativas.md`).
- **Autonomía (Ander, 2026-11-03):** resolver lo pendiente sin consultarle salvo permisos reales (aprobar diálogos,
  cerrar procesos ajenos en acr12, cosas que solo él puede hacer). Antes sí se le consultan cambios de **diseño,
  licencias o alcance**.
- **Fusión de PRs:** gestión fusiona con CI en verde tras revisar; **squash**; actualizar ramas con **merge de main,
  nunca rebase ni force-push** (el force-push está bloqueado).
- **Diseño de la app** (`docs/design/`, PR #28 y #82): seis modos fijos (Leer, Comentar, Rellenar y firmar,
  Organizar, Editar, Proteger) + «Herramientas» + Ctrl+K; tareas en el inspector derecho y guardar copia por defecto;
  estilo «grafito y tinta» (acento `#0f6b62`), Segoe UI Variable, iconos Lucide; paleta Herramientas no modal; los
  seis modos no se quitan ni reordenan en v1.
- **Instalador propio** `pliego-setup` (Rust + wry), por usuario sin UAC, lanzador `Pliego.exe` con carpetas
  `app-<versión>` y `current`/`pending`; MSI solo para empresas (W10).
- **Actualizaciones:** servidor estático (Caddy en un VPS que pondrá Ander), manifiesto firmado Ed25519
  `v1/<canal>/manifest.json`. La clave secreta de producción la genera Ander sin conexión: **nunca al repo ni al
  VPS**. Con las actualizaciones «Desactivadas» la app no hace ninguna petición.
- **Memoria en reposo (W01, Ander, 2026-11-04, revisada 2026-11-06):** se mide aparte la memoria propia de Pliego (app + worker): 150 MB.
  La cifra del Administrador de tareas contando WebView2 sube de 400 a **500 MB** (conjunto de trabajo, PDF de 100 páginas en reposo).
  Ambos límites los comprueba `tools/bench-memoria-webview.ps1`.
- **Bates continua en lote:** en las acciones en lote la numeración Bates sigue entre archivos y está activada por
  defecto (#103), como se espera en un expediente.
- **TLS:** https con rustls + proveedor ring (licencias compatibles con AGPL). Dependencias RustCrypto en la
  generación estable (cms 0.2, x509-cert 0.2, der 0.7, rsa 0.9, p256/p384 0.13); migrar a rsa 0.10 cuando sea
  estable (RUSTSEC-2023-0071).
- **W08:** linealización, subconjunto de fuentes y JBIG2/CCITT aplazados (mupdf-rs no los expone).
- **W18:** modelo recomendado Qwen3 4B Instruct Q4_K_M (10/10 en la prueba de precisión; Phi-3.5 9/10; Qwen2.5 1/10).
- **Borradores para mupdf-rs** (`borradores/`): los publica Ander si quiere; ningún hilo publica fuera del repo sin
  su visto bueno.

## Pendiente de Ander

1. ~~VPS de actualizaciones~~: hecho (2026-11-04). **Pendiente:** copia sin conexión de `D:\dev-cache\release-keys`
   en otro equipo (no en USB), la hace Ander.
2. ~~W11: abrir en Reader los PDFs editados~~: **revisión manual aprobada por Ander (2026-11-04 20:27)** para las líneas
   de `revision-manual-ander.md` existentes entonces (W11, W03, W07, W08, W09, W22, W04, W14, W21). Siguen abiertas las que
   dependen de la beta de la ola 4 (W23, W25 y las que añadan W26-W29).
3. ~~W20 (y W03): control de pantalla~~: no funciona desde Remote Control; lo visual lo hace Ander a mano. Ander autorizó
   (2026-11-04) que gestión pruebe sola en acr12 la beta nueva y los PDFs cifrados de W07 en lo que no necesite pantalla.
4. ~~Criterio de memoria de W01~~: **decidido por Ander (2026-11-04, revisado 2026-11-06)**: 150 MB para app + worker
   (medidos aparte) y techo de 500 MB en el Administrador de tareas contando WebView2.
5. **DNIe/FNMT (W05):** acr12 no tiene lector de tarjetas conectado (#106); conectar uno o descartar la prueba.
6. ~~Modelos GGUF de W18 en C:~~: **Ander autorizó moverlos a D: (2026-11-04)**; lo hace W18.
7. ~~Excluir `D:\dev-cache` del antivirus~~: no hace falta. La causa del indexado lento era `system-fonts` de MuPDF
   (font-kit enumera las fuentes en cada búsqueda); resuelto en #124 con un `FontLoader` propio.
8. En la cuenta nueva: los pasos manuales de [`prompt-nuevo-proyecto.md`](prompt-nuevo-proyecto.md).
