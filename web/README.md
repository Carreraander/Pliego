# web/ · sitio pdfpliego.com

Sitio estático de Pliego: HTML y CSS a mano, sin compilación, sin JavaScript y sin peticiones a terceros.
Diseño y estrategia SEO: [`docs/design/landing-y-seo.md`](../docs/design/landing-y-seo.md).

| Archivo | Qué es |
| --- | --- |
| `index.html` | Landing (con JSON-LD `SoftwareApplication` + `FAQPage`) |
| `descargar/index.html` | Descarga y requisitos (hoy, aviso de prelanzamiento) |
| `privacidad/index.html` | Privacidad del sitio y del programa |
| `404.html` | Página no encontrada |
| `assets/site.css` | Estilos; tokens copiados de `docs/design/tokens.css` |
| `favicon.svg`, `favicon.ico`, `apple-touch-icon.png`, `og.png`, `site.webmanifest` | Iconos e imagen para redes |
| `robots.txt`, `sitemap.xml` | Rastreo |
| `Caddyfile.propuesta` | Bloque de Caddy para el VPS (no desplegado) |

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
3. Añadir el bloque de `Caddyfile.propuesta` al Caddyfile del VPS y `systemctl reload caddy`.
4. Comprobar: `curl -I https://pdfpliego.com/`, Rich Results Test de Google, validator.schema.org.

## Al publicar la primera versión de Pliego

- `index.html`: quitar la píldora «En desarrollo…» de la portada.
- `descargar/index.html`: cambiar el bloque marcado `PRELANZAMIENTO` por el botón al instalador, versión, tamaño y
  SHA-256.
- JSON-LD de `index.html`: añadir `softwareVersion`, `downloadUrl` y `fileSize`.
- `sitemap.xml`: actualizar `lastmod`.

## Reglas para cambiar el texto

- Solo afirmaciones que estén en `main` y probadas; nada de cifras sin medir en acr12 (ver §3 del documento de diseño).
- Ni logos, ni colores, ni capturas, ni vocabulario de Adobe. Acrobat solo en uso descriptivo y con el aviso de marcas.
- Si cambias una pregunta frecuente, cambia también su copia en el JSON-LD (el texto debe ser idéntico).
- `og.png` y `apple-touch-icon.png` se generaron con Playwright a partir de una plantilla HTML; si cambia el logo,
  regenéralas.
