# Publicar versiones de Pliego

Guía para quien publica (Ander). Diseño y reglas de seguridad: `docs/design/actualizaciones.md`. Servidor:
`deploy/updates/README.md`. Herramienta: `pliego-release` (`cargo run -p pliego-release -- <orden>`).

## 1. Crear la clave de firma (una sola vez, sin conexión)
En un equipo de confianza, preferiblemente desconectado de la red:
```
pliego-release keygen --out D:\claves-pliego
```
Genera `pliego-update.key` (SECRETA) y `pliego-update.pub` (`ed25519:…`).
- La clave secreta **nunca** va al repositorio ni al VPS. Guárdala en dos copias fuera de línea (gestor de contraseñas
  y memoria USB cifrada). Su contenido, en el secreto `PLIEGO_UPDATE_SIGNING_KEY` del entorno protegido `release`.
- Haz una **segunda clave de reserva** (`--name pliego-reserva`) y guárdala solo sin conexión: sirve para rotar.
- Las claves públicas (activa y reserva) van en la variable `PLIEGO_UPDATE_PUBKEYS` (separadas por comas) al compilar,
  o en `BUILTIN_RELEASE_KEYS` de `crates/pliego-update/src/keys.rs`.

## 2. Compilar la versión oficial
```
set PLIEGO_UPDATE_URL=https://actualizaciones.<tu dominio>
set PLIEGO_UPDATE_PUBKEYS=ed25519:<activa>,ed25519:<reserva>
installer\setup\build.ps1 -Version 0.5.0 -Out dist
```
Sin `PLIEGO_UPDATE_URL` o sin claves, la app no hace ninguna petición de red. Los binarios de desarrollo, por tanto,
nunca buscan actualizaciones.

## 3. Publicar
```
pliego-release publish --root dist\actualizaciones --channel stable --version 0.5.0 ^
  --package dist\pliego-0.5.0-x86_64.paq --setup dist\Pliego-Setup-0.5.0-x86_64.exe ^
  --notes-es @notas\0.5.0.es.md --rollout 20 --key D:\claves-pliego\pliego-update.key
pliego-release verify --root dist\actualizaciones --pubkey D:\claves-pliego\pliego-update.pub
rsync -av --delete-after dist/actualizaciones/ pliego@actualizaciones.<dominio>:/
```
Con la CI: empuja la etiqueta `v0.5.0`; el flujo `release` compila en Windows, espera tu aprobación en el entorno
`release`, firma, crea la Release de GitHub (borrador) y sube al VPS (propuesta en `deploy/updates/release.yml.propuesta`).
Las etiquetas con sufijo (`v0.6.0-beta.1`) van al canal `beta`.

## 4. Ampliar el despliegue
Publica la misma versión con más porcentaje: `20 → 50 → 100` (`--rollout 50`, etc.). Vigila los informes de fallos
entre pasos. Con `--rollout 100` todas las instalaciones la ven.

## 5. Volver a firmar (cada mes)
El manifiesto caduca a los 90 días por defecto; la app deja de fiarse 24 h después. Sin publicar nada nuevo:
```
pliego-release resign --root dist\actualizaciones --channel stable --key D:\claves-pliego\pliego-update.key
```
El flujo programado (día 1 de cada mes) lo hace solo cuando exista `UPDATE_SSH_HOST`.

## 6. Retirar una versión mala
Publica una versión corregida o repite la última buena con `--block 0.5.0` (y, si hace falta, `--critical` o
`--min-supported 0.5.1`). Las instalaciones en una versión bloqueada pasan a "obligatoria" y se les ofrece actualizar.
No se baja nunca a nadie de versión: se publica una versión nueva con número mayor.

## 7. Rotar claves
- *Clave activa comprometida:* firma con la **reserva** una versión (`--key` de la reserva) que traiga compilada una
  clave de reserva nueva y sin la comprometida. Hasta que esa versión llegue, solo la firma de reserva es válida:
  publica el manifiesto firmado con ambas durante la transición (`--key` se puede repetir).
- *Rotación programada (anual):* igual, sin urgencia.

## 8. Probar sin VPS
```
pliego-release serve --root dist\actualizaciones
set PLIEGO_UPDATE_URL=http://127.0.0.1:8787
```
La app solo acepta HTTP en `localhost`, `127.0.0.1` y `[::1]`.
