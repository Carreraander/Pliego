#!/usr/bin/env bash
# Prepara un VPS Debian/Ubuntu para servir las actualizaciones de Pliego. Idempotente: se puede repetir.
#
# Uso (como root):  ./setup-vps.sh actualizaciones.ejemplo.es "ssh-ed25519 AAAA… despliegue-pliego"
#   $1  dominio que apunta al VPS
#   $2  clave SSH pública de despliegue (la que usa la CI para subir con rsync)
#
# Crea el usuario `pliego` (sin shell: solo puede ejecutar rsync dentro de la carpeta), la carpeta
# /srv/pliego-actualizaciones y deja Caddy sirviéndola con HTTPS. No guarda ningún secreto de firma.
set -euo pipefail

DOMAIN="${1:?Falta el dominio. Uso: $0 <dominio> \"<clave ssh pública>\"}"
SSH_KEY="${2:?Falta la clave SSH pública de despliegue}"
ROOT_DIR=/srv/pliego-actualizaciones
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

[ "$(id -u)" -eq 0 ] || { echo "Ejecuta este script como root." >&2; exit 1; }
case "$SSH_KEY" in ssh-ed25519\ *|ssh-rsa\ *|ecdsa-sha2-*) ;; *) echo "La clave SSH no parece válida." >&2; exit 1 ;; esac
case "$DOMAIN" in *[!a-zA-Z0-9.-]*|"") echo "Dominio no válido." >&2; exit 1 ;; esac

export DEBIAN_FRONTEND=noninteractive
if ! command -v caddy >/dev/null; then
  apt-get update -qq
  apt-get install -y -qq debian-keyring debian-archive-keyring apt-transport-https curl gpg rsync
  curl -fsSL https://dl.cloudsmith.io/public/caddy/stable/gpg.key | gpg --dearmor --yes -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
  curl -fsSL https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt >/etc/apt/sources.list.d/caddy-stable.list
  apt-get update -qq
  apt-get install -y -qq caddy
fi
command -v rsync >/dev/null || apt-get install -y -qq rsync

# Usuario de despliegue: sin contraseña ni shell interactiva; solo rsync, restringido a la carpeta.
id pliego >/dev/null 2>&1 || useradd --system --create-home --home-dir /home/pliego --shell /bin/sh pliego
install -d -o pliego -g pliego -m 755 "$ROOT_DIR"
install -d -o pliego -g pliego -m 700 /home/pliego/.ssh
# rrsync (viene con rsync) limita la subida a la carpeta de actualizaciones.
RRSYNC="$(command -v rrsync || true)"
if [ -z "$RRSYNC" ] && [ -f /usr/share/doc/rsync/scripts/rrsync.gz ]; then
  gunzip -c /usr/share/doc/rsync/scripts/rrsync.gz >/usr/local/bin/rrsync && chmod 755 /usr/local/bin/rrsync
  RRSYNC=/usr/local/bin/rrsync
fi
[ -n "$RRSYNC" ] || { echo "No se encontró rrsync (paquete rsync)." >&2; exit 1; }
echo "restrict,command=\"$RRSYNC $ROOT_DIR\" $SSH_KEY" >/home/pliego/.ssh/authorized_keys
chown pliego:pliego /home/pliego/.ssh/authorized_keys && chmod 600 /home/pliego/.ssh/authorized_keys

# Caddy: copia el Caddyfile cambiando el dominio y recarga.
sed "s/actualizaciones\.ejemplo\.es/$DOMAIN/" "$HERE/Caddyfile" >/etc/caddy/Caddyfile
install -d -o caddy -g caddy /var/log/caddy
caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile
systemctl enable --now caddy
systemctl reload caddy

echo "Listo. Comprueba:  curl -I https://$DOMAIN/v1/stable/manifest.json  (404 hasta la primera publicación)"
echo "Sube con:  rsync -av --delete-after dist/actualizaciones/ pliego@$DOMAIN:/"
