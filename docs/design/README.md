# Diseño de la app de escritorio de Pliego

Versión 1 (2026-11-03), hilo de diseño "Diseñar app de escritorio de Pliego". Sustituye al boceto del PR #1.

| Documento | Contenido |
| --- | --- |
| [prototipo.html](prototipo.html) | Prototipo navegable: ventana, modos, paleta de herramientas, Ctrl+K, ajustes de actualizaciones, instalador y sistema visual. Ábrelo en un navegador. Publicado también como artefacto: https://claude.ai/artifact/Pin9i2YGZRQUuDdft48FVR |
| [ux-y-navegacion.md](ux-y-navegacion.md) | Principios, anatomía de la ventana, pantallas, modelo de herramientas (modo, tarea, asistente), mapa de todas las herramientas por flujo, patrones, atajos, accesibilidad, glosario y el contrato `ToolManifest` para integrar herramientas |
| [sistema-visual.md](sistema-visual.md) | Color, tipografía, espaciado, iconos, logo, movimiento |
| [tokens.css](tokens.css) | Tokens de diseño (`--pl-*`), fuente de verdad. Se copia a `app/src/styles/tokens.css` |
| [instalador.md](instalador.md) | Instalador propio `pliego-setup`: estructura en disco con lanzador, paquete `.paq`, pantallas, registro en Windows, línea de órdenes, desinstalación, firma de código |
| [landing-y-seo.md](landing-y-seo.md) | Web pdfpliego.com: diseño de la landing, afirmaciones permitidas, palabras clave, metadatos y schema. Código en `web/` |
| [actualizaciones.md](actualizaciones.md) | Protocolo de actualizaciones firmadas (Ed25519), configuración, experiencia de usuario, aplicar y volver atrás, publicar, VPS |

Código de referencia ya escrito:
- `crates/pliego-update`: cliente de actualizaciones (manifiesto firmado, canales, despliegue gradual, anti-reproducción,
  caducidad, descarga verificada). Sin Tauri ni Windows: lo usan la app y el instalador.
- `crates/pliego-release`: `keygen`, `publish`, `resign`, `verify`, `serve` (servidor local de pruebas) y `check`.

Flujos de implementación (Sonnet): W19–W22 en `docs/workstreams/`.
