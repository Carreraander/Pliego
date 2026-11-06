# web/ · sitio pdfpliego.com

Sitio estático de Pliego: HTML y CSS a mano, sin compilación y sin peticiones a terceros.
El único script es `assets/demo.js`, la demo interactiva de la portada (mejora progresiva: sin él se ve la ventana
estática).
Diseño y estrategia SEO: [`docs/design/landing-y-seo.md`](../docs/design/landing-y-seo.md).

| Archivo | Qué es |
| --- | --- |
| `index.html` | Landing en inglés (con JSON-LD `SoftwareApplication` + `FAQPage`) |
| `es/index.html` | Landing en español (JSON-LD `WebPage` + `FAQPage` en español) |
| `download/index.html`, `es/descargar/index.html` | Descarga y requisitos (hoy, aviso de prelanzamiento) |
| `privacy/index.html`, `es/privacidad/index.html` | Privacidad del sitio y del programa |
| `404.html` | Página no encontrada (inglés, con enlace al español) |
| `assets/site.css` | Estilos; tokens copiados de `docs/design/tokens.css` |
| `assets/demo.js` | Demo interactiva de la ventana de Pliego (todo simulado, sin dependencias; idioma según `<html lang>`) |
| `favicon.svg`, `favicon.ico`, `apple-touch-icon.png`, `og.png`, `og-es.png`, `site.webmanifest` | Iconos e imágenes para redes |
| `robots.txt`, `sitemap.xml` | Rastreo |
| `Caddyfile.propuesta` | Bloque de Caddy del VPS (`/etc/caddy/sites.d/pdfpliego.caddy`), con las redirecciones de las rutas antiguas |

## Ver en local

```
cd web && python3 -m http.server 8000   # http://localhost:8000
```

Las rutas son absolutas (`/assets/site.css`), así que hay que servir la carpeta; abrir el archivo con doble clic no
carga los estilos.

## Publicar (solo con el visto bueno de Ander)

1. DNS: `A`/`AAAA` de `pdfpliego.com` y `www.pdfpliego.com` al VPS.
2. Copiar `web/` (sin `README.md` ni `Caddyfile.propuesta`) a `/srv/pdfpliego.com` en el VPS:
   `rsync -a --delete --exclude README.md --exclude Caddyfile.propuesta web/ vps:/srv/pdfpliego.com/`
3. El VPS es compartido: **no** tocar el Caddyfile global. Copiar `Caddyfile.propuesta` a
   `/etc/caddy/sites.d/pdfpliego.caddy` (el global importa `sites.d/*.caddy`, como deja
   `deploy/updates/setup-vps.sh`), `caddy validate --config /etc/caddy/Caddyfile` y `systemctl reload caddy`.
   Ojo: la cabecera HSTS lleva `includeSubDomains`, así que todos los subdominios de pdfpliego.com
   (incluido `actualizaciones.pdfpliego.com`) deben ir por HTTPS.
4. Comprobar: `curl -I https://pdfpliego.com/`, Rich Results Test de Google, validator.schema.org.

## Al publicar la primera versión de Pliego

- `index.html` y `es/index.html`: quitar la píldora «Pre-release…» / «Prelanzamiento…» de la portada.
- `download/index.html` y `es/descargar/index.html`: cambiar el bloque marcado `PRE-RELEASE` / `PRELANZAMIENTO` por el
  botón al instalador, versión, tamaño y SHA-256.
- JSON-LD de `index.html`: añadir `softwareVersion`, `downloadUrl` y `fileSize`.
- Buy Me a Coffee: sustituir el `href` marcado `PENDING (Ander)` / `PENDIENTE (Ander)` por la página real.
- `sitemap.xml`: actualizar `lastmod`.

## Reglas para cambiar el texto

- Solo afirmaciones que estén en `main` y probadas; nada de cifras sin medir en acr12 (ver §3 del documento de diseño).
- Ni logos, ni colores, ni capturas, ni vocabulario de Adobe. Acrobat solo en uso descriptivo y con el aviso de marcas.
- Si cambias una pregunta frecuente, cambia también su copia en el JSON-LD (el texto debe ser idéntico), y haz el
  mismo cambio en las dos lenguas.
- Sin afirmaciones firmes de licencia (va a cambiar): «gratis en casa y en la empresa» y «código público».
- `og.png`, `og-es.png` y `apple-touch-icon.png` se generaron con Playwright a partir de una plantilla HTML; si cambia
  el logo o el titular, regenéralas.
