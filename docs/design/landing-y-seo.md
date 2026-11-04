# pdfpliego.com · landing y SEO

Estado: propuesta v1 (2026-11-04), hilo «Diseñar la landing y SEO de pdfpliego.com». Dueño: gestión.
Código: [`web/`](../../web/) (HTML y CSS estáticos, sin compilación). No está desplegado: publicar en el VPS
necesita el visto bueno de Ander.

## 1. Objetivo y público

- **Trabajo de la página:** que alguien que busca un programa PDF gratis para Windows entienda en 10 segundos qué
  hace Pliego, se fíe (libre, local, sin trampas) y lo descargue.
- **Público:** personas y pequeñas oficinas en España y Latinoamérica que hoy usan un lector gratuito y pagan o
  esquivan una suscripción para firmar, combinar, comprimir o hacer OCR; gestorías y despachos que firman con
  certificado FNMT; administración y educación que valoran software libre y sin nube.
- **Idioma:** español primero (`/`). Inglés más adelante en `/en/` con `hreflang` recíproco (§5.4).

## 2. Diseño

Mismo sistema que la app: «grafito y tinta» ([sistema-visual.md](sistema-visual.md)), con los valores de
[tokens.css](tokens.css) copiados en `web/assets/site.css` (el sitio no depende de la app).

- **Idea:** el sitio es un pliego. Las secciones se separan con líneas de corte discontinuas, no con tarjetas, y la
  portada presenta la ventana real de la app dentro de sus **marcas de corte y registro** de imprenta. Es el detalle
  que solo tiene este producto, igual que el doblez del logo y la pestaña.
- **Portada:** titular «Todo lo que haces con un PDF, en tu equipo y gratis.», entradilla, dos botones (Descargar
  para Windows, Ver el código) y cuatro datos (Windows 10 y 11, sin administrador, AGPL-3.0, sin internet). Debajo,
  la ventana de Pliego en el modo **Rellenar y firmar** con un contrato firmado, el asistente de firma en el inspector
  y la barra de estado «A4 · 210 × 297 mm · Firmado · válido». Está hecha en HTML/CSS, no es una captura: pesa poco,
  sigue el tema claro/oscuro y no se queda vieja si cambia un color.
- **Secciones:** Seis modos (rejilla 3×2 con atajo `Alt+n`) → Ctrl+K (paleta con la búsqueda «comprimir») →
  Herramientas (índice por grupos) → Privacidad (tres compromisos) → Software libre (tabla de hechos: 0 €, sin marcas
  de agua, sin límites, uso en empresas, MuPDF, licencia) → Preguntas → Llamada final → Pie con avisos de marcas.
- **Tipografía:** la del sistema (Segoe UI Variable en Windows, que es justo el público). Sin fuentes web.
- **Tema:** claro y oscuro con `prefers-color-scheme`; alto contraste con `forced-colors`; `prefers-reduced-motion`.
- **Demo que se puede usar** (`web/assets/demo.js`, a petición de Ander el 2026-11-04): la ventana de la portada es
  interactiva. Pestañas (cambiar, cerrar, nueva pestaña con la pantalla de Inicio), los seis modos con sus
  herramientas (resaltar líneas con colores, notas y sellos donde se haga clic, rotar/duplicar/eliminar páginas,
  editar el título de la página, marca de agua, Bates, redactar y aplicar, contraseña), Herramientas, Ctrl+K con
  búsqueda y flechas, paneles del raíl (miniaturas, marcadores, comentarios, firmas), zoom, `Alt+1…6` y el asistente
  de firma completo hasta «Firmar y guardar». Las tareas (Comprimir, OCR, PDF/A, Comparar) muestran progreso en la
  barra de estado y avisan de la copia guardada. Todo es simulado: no se abre ni se envía ningún archivo.
- **Mejora progresiva:** sin JavaScript se ve la ventana estática. Las preguntas usan `<details>`. Cero peticiones a
  terceros: ni fuentes, ni analítica, ni CDN.
  Es coherente con el mensaje de privacidad y deja el sitio en lo más alto de Core Web Vitals (HTML 30 KB, CSS 20 KB).
- **Accesibilidad:** enlace «Saltar al contenido», jerarquía h1→h2→h3, foco visible, iconos con `aria-hidden`, las
  maquetas de la app como `role="img"` con descripción, contraste ≥ 4,5:1 (mismos pares que la app).

### Páginas

| Ruta | Contenido | Indexar |
| --- | --- | --- |
| `/` | Landing | Sí |
| `/descargar/` | Requisitos, instalación, comprobar SHA-256, actualizaciones. Hoy muestra el aviso de prelanzamiento | Sí |
| `/privacidad/` | Qué datos tratan el sitio y el programa (ninguno identificativo) | Sí |
| `/404.html` | Página no encontrada | No |

## 3. Afirmaciones: qué se dice y qué no

Revisado contra `docs/handoff/flujos/` el 2026-11-04. Regla: solo lo que está en `main`; lo pendiente de verificar
en acr12 se dice sin cifras ni superlativos.

- **Sí:** gratis, AGPL-3.0, sin marcas de agua ni límites, sin cuenta, sin telemetría, OCR local, firma PAdES con
  certificado del almacén de Windows o `.p12/.pfx` (W05 #106), por usuario sin administrador (W21), actualizaciones
  firmadas y desactivables (W22), las funciones de los seis modos y de Herramientas.
- **No (todavía):**
  - **DNIe:** no se ha probado con lector (W05). No aparece en la página hasta que se pruebe; entonces se añade a
    «Rellenar y firmar», a la pregunta de firma y como palabra clave (§4).
  - **Cifras** de velocidad, memoria o tamaño del instalador (W01, W10 sin medir en acr12).
  - **Asistente IA local** (W18 sin compilar en Windows con `--features llama`).
  - **Comparativas** del tipo «mejor/más rápido que Acrobat». Solo se nombra Acrobat en la pregunta «¿Es una
    alternativa a Adobe Acrobat?» (uso descriptivo), con la frase «proyecto independiente, sin relación con Adobe» y
    el aviso de marcas en el pie. Nada de logos, colores, capturas ni vocabulario de Adobe.
- **Prelanzamiento:** la píldora «En desarrollo · primera versión de prueba muy pronto» y el aviso de `/descargar/`
  son verdad hoy. Al publicar la primera versión se cambian (ver `web/README.md`).

## 4. Palabras clave

Sin herramienta de volúmenes en este hilo: el orden es por intención y por lo alcanzable, no por cifras. Antes de
lanzar conviene validarlas con Google Search Console (tras una semana publicado) o con Keyword Planner.

Los resultados de «unir pdf», «comprimir pdf» o «pdf a word» a secas los dominan servicios web; ahí no se compite de
frente. Pliego gana en las búsquedas con **programa / Windows / gratis / sin internet / libre / certificado**.

| Prioridad | Grupo | Palabras clave | Página |
| --- | --- | --- | --- |
| 1 | Marca | pliego, pliego pdf, pdfpliego | `/` |
| 1 | Programa gratis | programa pdf gratis, editor pdf gratis windows, programa para editar pdf gratis, editor de pdf gratis sin marca de agua | `/` |
| 1 | Alternativa | alternativa gratis a acrobat, alternativa a adobe acrobat gratis, alternativa libre a acrobat | `/` (pregunta) y futura `/alternativa-libre/` |
| 2 | Firma | firmar pdf con certificado digital, firmar pdf con certificado fnmt, firma pades gratis, validar firma pdf | futura `/firmar-pdf-con-certificado/` |
| 2 | Local / privacidad | editor pdf sin internet, programa pdf sin conexión, editar pdf sin subirlo | `/` (privacidad) |
| 2 | OCR | ocr pdf gratis windows, convertir pdf escaneado en texto, pdf escaneado buscable | futura `/ocr-pdf/` |
| 3 | Tareas | unir pdf sin internet, comprimir pdf sin perder calidad programa, pdf a word sin internet, rellenar formulario pdf, proteger pdf con contraseña, quitar datos personales de un pdf | futuras páginas por tarea |
| 3 | Software libre | editor pdf software libre, editor pdf código abierto windows | `/` (sección libre) |
| 3 | Profesional | numeración bates pdf, pdf/a validar, redactar pdf | futuras páginas por tarea |
| Más tarde | DNIe | firmar pdf con dnie | solo cuando W05 lo pruebe |

## 5. SEO técnico

### 5.1 Metadatos de `/`

- `<title>` (49 caracteres): `Pliego · Programa PDF gratis y libre para Windows`
- `description` (≈155): «Pliego es un programa PDF gratuito y libre para Windows: lee, comenta, rellena, firma con
  certificado digital, organiza, aplica OCR y protege tus PDF sin nube ni cuenta.»
- `canonical`, `hreflang="es"` y `x-default`, `robots: index, follow, max-image-preview:large`.
- Open Graph y Twitter: `og.png` 1200×630 (logo, titular, la hoja firmada entre marcas de corte), `og:locale es_ES`.
- `theme-color` por tema, `favicon.svg` (cambia con el tema), `favicon.ico`, `apple-touch-icon.png`, `site.webmanifest`.

Cada página interior tiene su título, descripción y canonical propios. `404.html` lleva `noindex`.

### 5.2 Datos estructurados (JSON-LD en `/`)

Un `@graph` con:
- `WebSite`.
- `SoftwareApplication`: `applicationCategory: BusinessApplication`, `operatingSystem: Windows 10, Windows 11`,
  `offers` a 0 EUR, `isAccessibleForFree`, `license` (AGPL), `featureList`, `sameAs` al repositorio.
  Al publicar la primera versión: añadir `softwareVersion`, `downloadUrl` y `fileSize`.
- `FAQPage` con las mismas siete preguntas que se ven en la página (texto idéntico).

Notas: Google solo muestra la ficha enriquecida de software si hay `aggregateRating` o `review`. **No se inventan
valoraciones**: se añadirán si algún día hay reseñas reales verificables. Desde 2023 Google enseña las FAQ enriquecidas
casi solo a webs de administración y salud; el marcado se deja porque es correcto y lo leen otros buscadores y los
asistentes. Validar con el Rich Results Test y validator.schema.org antes de publicar.

### 5.3 Rastreo

- `robots.txt` abierto con `Sitemap:`; `sitemap.xml` con las tres páginas indexables y `lastmod`.
- URLs en minúsculas, en español, con barra final (`/descargar/`). `www` redirige a la raíz (301).
- `try_files` en Caddy para servir `/ruta/` → `/ruta/index.html`; 404 real con `404.html`.

### 5.4 Más adelante

- **Páginas por tarea** (una por grupo de la tabla §4): misma plantilla que `/descargar/`, con un h1 que responde a
  la búsqueda («Firmar un PDF con certificado digital en Windows»), pasos reales con capturas de la app, requisitos
  y preguntas propias. Solo cuando la función esté verificada en acr12.
- **`/alternativa-libre/`**: tabla de qué tareas cubre Pliego y cuáles no, sin comparar con otros productos.
- **Novedades** (`/novedades/`): notas de cada versión; da contenido fresco y enlaces.
- **Inglés** en `/en/` con `hreflang` recíproco cuando haya traducción de la app.
- **Enlaces entrantes** (lo que más mueve el SEO de un programa libre): repositorio de GitHub con la web en
  «About», alternativeto.net, listas de software libre de administraciones y universidades, Softonic/Uptodown en
  español, foros de gestorías. Todo esto es publicar fuera del repo: **lo decide Ander**.
- **Search Console y Bing Webmaster Tools:** verificar el dominio por DNS (registro TXT) y enviar el sitemap.
  Lo hace Ander.

## 6. Rendimiento y seguridad

- Objetivo: LCP < 1,5 s en 4G, CLS 0. Único script: la demo (≈33 KB sin comprimir, `defer`, propio). Las imágenes del sitio son solo `og.png` y los iconos.
- Cabeceras en `web/Caddyfile.propuesta`: HSTS, CSP estricta (`script-src 'self'`), `nosniff`,
  `Referrer-Policy`, `Permissions-Policy`, compresión zstd/gzip, HTML sin caché y estáticos una semana.
- Registro de acceso 7 días, igual que dice `/privacidad/`.

## 7. Pendiente de decidir (Ander)

1. Desplegar en el VPS (copiar `web/Caddyfile.propuesta` a `/etc/caddy/sites.d/pdfpliego.caddy`, no al Caddyfile
   global porque el VPS es compartido, y `web/` a `/srv/pdfpliego.com`). HSTS con `includeSubDomains` obliga a
   HTTPS en todos los subdominios.
2. Responsable del sitio en `/privacidad/` (nombre y contacto): hoy remite a GitHub.
3. Dónde se aloja el instalador (`/descargar/` del VPS o *Releases* de GitHub) cuando exista la primera versión.
4. Alta en Search Console/Bing y en directorios de software (§5.4).
