# Servidor de actualizaciones de Pliego

El servidor es una carpeta de **archivos estáticos** servida por Caddy. No hay base de datos ni código: lo que
protege a los usuarios son las firmas Ed25519 de los manifiestos, no el VPS. Diseño completo en
`docs/design/actualizaciones.md`; cómo publicar una versión en `docs/releasing.md`.

## Qué hay aquí
- `Caddyfile`: configuración de Caddy (HTTPS automático, manifiestos sin caché, paquetes inmutables).
- `setup-vps.sh`: prepara el VPS (idempotente). Crea el usuario `pliego` (solo `rsync` confinado a la carpeta), la
  carpeta `/srv/pliego-actualizaciones` e instala Caddy.
- `release.yml.propuesta`: flujo de GitHub Actions de publicación por etiqueta `v*` (lo debe incorporar W10 en
  `.github/workflows/`).

## Cuando tengas el VPS (checklist)
1. Contrata un Linux pequeño (Debian 12 o Ubuntu 24.04, 1 vCPU, 1 GB) y un subdominio, por ejemplo
   `actualizaciones.<tu dominio>`, con registro A/AAAA hacia el VPS. Abre los puertos 22, 80 y 443.
2. Genera (o reutiliza) una clave SSH solo para desplegar: `ssh-keygen -t ed25519 -f pliego-deploy -C despliegue-pliego`.
3. En el VPS, como root: `./setup-vps.sh actualizaciones.<dominio> "$(cat pliego-deploy.pub)"`.
4. Comprueba `https://actualizaciones.<dominio>/v1/stable/manifest.json` (404 hasta publicar).
5. En GitHub > Settings > Secrets del entorno `release`: `UPDATE_SSH_KEY` (la clave privada de despliegue),
   y `UPDATE_SSH_HOST` (el dominio). La clave de firma NO va a GitHub: se firma a mano (ver `docs/releasing.md`). Variable
   `PLIEGO_UPDATE_URL=https://actualizaciones.<dominio>`. Mientras no exista `UPDATE_SSH_HOST`, el paso `rsync` se omite.
6. Publica una versión y comprueba con `pliego-release check --url https://actualizaciones.<dominio> --pubkey …`.

## Probar sin VPS
```
pliego-release serve --root dist/actualizaciones      # http://127.0.0.1:8787
set PLIEGO_UPDATE_URL=http://127.0.0.1:8787            # Windows (cmd); la app usa HTTP solo en loopback
```
Mudar de proveedor: `rsync` de la carpeta al servidor nuevo, repuntar el DNS y listo. Los clientes no cambian
porque la URL base es el dominio, no la IP.

## VPS compartido

Si el VPS ya sirve otros sitios con Caddy, `setup-vps.sh` **no toca** `/etc/caddy/Caddyfile`: escribe solo
`/etc/caddy/sites.d/pliego.caddy`, que el Caddyfile existente debe importar con
`import /etc/caddy/sites.d/*.caddy` (si no lo hace, el script se detiene y lo explica). Antes de recargar valida la
configuración completa y, si falla, deshace el cambio. El `Caddyfile` del repositorio solo admite GET/HEAD, añade
HSTS y `nosniff`, oculta la cabecera `Server` y no lista directorios.
