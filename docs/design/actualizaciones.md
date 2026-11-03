# Pliego · Autoactualizaciones

Estado: propuesta v1 (2026-11-03) con prototipo funcionando: crates `pliego-update` (cliente) y `pliego-release`
(publicación y servidor de pruebas). Ander pondrá el VPS más adelante; todo es configurable y se prueba en local.

## 1. Resumen
- El servidor solo sirve **archivos estáticos**: no hay base de datos ni código en el VPS. Caddy con HTTPS
  automático basta. Se puede cambiar de proveedor copiando una carpeta.
- Cada canal (`stable`, `beta`) publica un **manifiesto firmado con Ed25519**. La app trae las claves públicas
  compiladas; sin una firma válida no se instala nada, aunque alguien tome el VPS o el DNS.
- La app descarga en segundo plano, **prepara la versión nueva al lado de la actual** y la activa al cerrar
  (o al pulsar "Reiniciar ahora"). Si la nueva no arranca, vuelve sola a la anterior.
- Sin telemetría: la petición solo lleva `User-Agent: Pliego/<versión> (windows)`. El despliegue gradual se decide en
  el propio equipo con un identificador aleatorio que nunca se envía.

## 2. Árbol del servidor

```
https://<servidor>/<ruta>/                     URL base (configurable)
  v1/stable/manifest.json                      manifiesto firmado del canal estable
  v1/beta/manifest.json
  packages/pliego-0.5.0-x86_64.paq             paquete para el actualizador
  packages/Pliego-Setup-0.5.0-x86_64.exe       instalador completo (descargas manuales)
  packages/Pliego-0.5.0-x86_64.msi             MSI corporativo
```
Los artefactos pueden estar en otro sitio (p. ej. GitHub Releases) con URL absoluta en el manifiesto: la integridad
la da el SHA-256 firmado, no el servidor.

## 3. Manifiesto

Sobre (lo que se publica): la carga útil va en base64 para firmar bytes exactos sin canonicalizar JSON.
```json
{
  "format": "pliego-update/1",
  "payload": "<base64 de la carga útil>",
  "signatures": [{ "key_id": "cb61b24f03c56bc8", "sig": "<base64 Ed25519>" }]
}
```
La firma es sobre `"pliego-update-manifest-v1\n" || payload` (separación de dominio).
`key_id` = 8 primeros bytes del SHA-256 de la clave pública, en hexadecimal.

Carga útil:
```json
{
  "schema": 1,
  "product": "pliego",
  "channel": "stable",
  "serial": 12,
  "issued_at": "2026-11-03T10:00:00Z",
  "expires_at": "2027-02-01T10:00:00Z",
  "latest": {
    "version": "0.5.0",
    "released_at": "2026-11-03T10:00:00Z",
    "notes": { "es": "- Firma con DNIe más rápida\n- Corrige …", "en": "…" },
    "notes_url": "https://…/novedades/0.5.0",
    "critical": false,
    "rollout_percent": 20,
    "min_windows_build": 19041,
    "artifacts": [
      { "kind": "package", "arch": "x86_64", "url": "packages/pliego-0.5.0-x86_64.paq", "size": 27312345, "sha256": "…" },
      { "kind": "setup",   "arch": "x86_64", "url": "packages/Pliego-Setup-0.5.0-x86_64.exe", "size": 29123456, "sha256": "…" }
    ]
  },
  "min_supported_version": "0.3.0",
  "blocked_versions": ["0.4.1"]
}
```

## 4. Reglas del cliente (implementadas en `pliego-update`, con pruebas)

| Regla | Protege de | Prueba |
| --- | --- | --- |
| Al menos una firma válida de una clave de confianza | Servidor o DNS comprometidos | `untrusted_key_is_rejected` |
| `product` y `channel` coinciden | Mezclar canales | `expired_wrong_channel_and_replay_are_rejected` |
| `expires_at` + 24 h > ahora | Ataque de congelación (servir siempre un manifiesto viejo) | ídem y e2e |
| `serial` ≥ último visto en ese canal | Reproducir un manifiesto antiguo firmado | `replayed_and_expired_manifests_are_rejected` |
| Nunca se ofrece versión ≤ instalada | Bajada de versión | `same_or_older_is_up_to_date` |
| Tamaño y SHA-256 del artefacto; se borra si no coincide | Paquete cambiado | `tampered_package_is_deleted` |
| HTTPS obligatorio salvo `localhost`/`127.0.0.1`/`[::1]` | Red hostil (defensa adicional) | `https_required_except_loopback` |
| Nombre de archivo saneado | Escritura fuera de la carpeta de descargas | `file_names_are_sanitised` |
| Obligatoria si `critical`, por debajo de `min_supported_version` o en `blocked_versions` | Quedarse en una versión peligrosa | `rollout_skip_and_mandatory` |
| Despliegue gradual: `sha256(install_id/versión) % 100 < rollout_percent` | Un fallo que llegue a todos a la vez | `bucket_is_stable_and_spread` |
| `min_windows_build` y arquitectura | Instalar algo que no arranca | `incompatible_windows_or_arch` |

Proxy: el de Windows (`win-system-proxy`) y las variables `HTTPS_PROXY`; TLS con el almacén de certificados de
Windows (SChannel vía `native-tls`).

## 5. Configuración

Precedencia (implementada en `pliego_update::resolve`, con pruebas):
1. **Directiva** `HKLM\SOFTWARE\Policies\Pliego\Updates`: `Disabled` (DWORD), `Url`, `Channel`, `Mode`
   (`automatic` | `notify-only` | `disabled`). En Ajustes se ve "Gestionado por tu organización" y no se puede tocar.
2. Variable `PLIEGO_UPDATE_URL` al ejecutar (pruebas con servidor local).
3. Ajustes del usuario (`%APPDATA%\Pliego\settings.json`: modo y canal).
4. URL compilada: `PLIEGO_UPDATE_URL` al compilar la versión oficial (la del VPS).

Claves públicas: `BUILTIN_RELEASE_KEYS` en `crates/pliego-update/src/keys.rs` o `PLIEGO_UPDATE_PUBKEYS` al compilar.
Sin URL o sin claves, las actualizaciones están desactivadas ("no configurado"): las compilaciones de desarrollo no
buscan nada.

Estado local: `%LOCALAPPDATA%\Pliego\update-state.json` (identificador de instalación, último serial por canal, última
comprobación, versión omitida). Descargas en `%LOCALAPPDATA%\Pliego\updates\`.

## 6. Experiencia de usuario

**Cuándo se comprueba:** 30 s después de arrancar y luego cada 6 h mientras la app está abierta, nunca más de una vez
por hora. Con conexión de uso medido de Windows, solo comprueba; no descarga sin permiso.

**Modos** (Ajustes > Actualizaciones):
- *Automático* (por defecto): descarga en segundo plano y prepara la versión. Se aplica al cerrar Pliego.
- *Solo avisar*: muestra que hay versión nueva; descarga al pulsar "Descargar".
- *Desactivadas*: no hace ninguna petición (tampoco para avisar de obligatorias).
Canal: Estable / Beta ("Recibe antes las novedades; puede tener fallos").

**Cuando hay una versión preparada:** aparece en la barra de título una pastilla discreta "Actualización lista"
(punto de acento + texto). Al pulsarla, popover con la versión, las notas y dos botones: **Reiniciar ahora** y
"Al cerrar Pliego". Si hay documentos sin guardar, "Reiniciar ahora" pide guardarlos primero (lista de documentos).
Pliego **nunca** se reinicia solo.

**Obligatorias** (crítica, versión no soportada o bloqueada): barra de aviso en el documento, en tono de aviso,
"Esta versión tiene un fallo de seguridad corregido en 0.5.1. [Reiniciar y actualizar]". Se puede cerrar la barra,
pero vuelve en el siguiente arranque. Con modo *Solo avisar* se descarga igualmente; con *Desactivadas* no se comprueba nada, así que tampoco se avisa.

**Ajustes > Actualizaciones** muestra: estado ("Pliego está al día · 0.4.0 · comprobado hace 2 h"; "Descargando
0.5.0 · 34 %"; "0.5.0 lista para instalar"), botón "Buscar ahora", modo, canal, "Novedades de esta versión",
historial de versiones instaladas en este equipo y, en "Avanzado", el servidor en uso (solo lectura) y "Ver
registro". Errores con texto llano: "No se pudo comprobar: sin conexión. Se reintentará en 1 hora."

**Tras actualizar:** al abrir, una tarjeta breve en Inicio "Novedades de Pliego 0.5.0" (se cierra y no vuelve).
Si hubo vuelta atrás: "Se volvió a 0.4.0 porque 0.5.0 no arrancaba. No se volverá a instalar 0.5.0."

## 7. Aplicar una actualización (instalación por usuario)
1. `pliego-update` descarga y verifica el `.paq` en `%LOCALAPPDATA%\Pliego\updates\`.
2. Lo extrae a `app-0.5.0.partial\`, comprueba cada archivo con `files.json` y renombra a `app-0.5.0\`.
3. Escribe `pending` = `0.5.0` (temporal + renombrar).
4. Al cerrar Pliego (o "Reiniciar ahora"), la app termina; en el siguiente arranque, o inmediatamente si se pidió
   reiniciar, el **lanzador** ve `pending`: escribe `current` = `0.5.0` de forma atómica, borra `pending`, arranca la
   nueva versión y lanza en segundo plano `app-0.5.0\pliego-setup.exe --post-update` (actualiza `DisplayVersion`,
   vuelve a registrar `pliego-shell`, renueva `Uninstall.exe` y, si cambió, el propio lanzador con
   renombrar-y-reemplazar).
5. **Salud:** la app escribe `app-0.5.0\healthy` tras 10 s funcionando. Si el lanzador arranca dos veces seguidas una
   versión sin `healthy` y el proceso termina antes de 10 s, vuelve a `current` = anterior, marca la versión como
   omitida y la app lo cuenta al usuario.
6. Limpieza: se conservan la versión actual y la anterior; el resto se borra.

Por máquina (Program Files): los pasos 2–4 los hace `pliego-setup --update --paq <archivo>` **elevado**; la app lo
lanza al pulsar "Reiniciar y actualizar" (escudo UAC en el botón). MSI: sin autoactualización (lo gestiona TI).
ZIP portátil: solo avisa y enlaza la descarga.

Deltas: fase 2 (`zstd --patch-from` por archivo contra la versión instalada; mismo manifiesto con `kind: "delta"` y
`from_version`).

## 8. Publicar una versión

```
# Una vez, en un equipo de confianza (Ander). La clave secreta NO va al repositorio ni al VPS.
pliego-release keygen --out D:\claves-pliego
#   → pliego-update.key (secreta), pliego-update.pub (pública); copiar la pública en keys.rs

# En cada versión (desde la CI o a mano):
pliego-release publish --root dist\actualizaciones --channel stable --version 0.5.0 ^
  --package pliego-0.5.0-x86_64.paq --setup Pliego-Setup-0.5.0-x86_64.exe ^
  --notes-es @notas\0.5.0.es.md --notes-en @notas\0.5.0.en.md --rollout 20 --key D:\claves-pliego\pliego-update.key
pliego-release verify --root dist\actualizaciones --pubkey D:\claves-pliego\pliego-update.pub

# Subir al VPS (solo archivos estáticos):
rsync -av --delete-after dist/actualizaciones/ pliego@vps:/srv/pliego-actualizaciones/

# Ampliar el despliegue: volver a publicar la misma versión con --rollout 100.
# Antes de que caduque (90 días por defecto) sin publicar nada:
pliego-release resign --root dist\actualizaciones --channel stable --key …\pliego-update.key
```

En la CI: un entorno de GitHub `release` protegido (aprobación de Ander) con la clave en un secreto; el flujo de
publicación construye, firma con Authenticode si hay certificado, ejecuta `publish` y sube por `rsync` con una clave
SSH de despliegue limitada a esa carpeta. Un trabajo mensual ejecuta `resign`.

Rotación de claves: la app trae dos claves (activa y de reserva sin conexión). Si la activa se compromete, se firma
con la de reserva una versión que trae una clave de reserva nueva y retira la comprometida.

## 9. VPS (cuando Ander lo tenga)

Requisitos: cualquier Linux pequeño (1 vCPU, 1 GB), dominio o subdominio (p. ej. `actualizaciones.<dominio>`).
`deploy/updates/` incluirá el `Caddyfile` y un script de preparación:

```
actualizaciones.ejemplo.es {
  root * /srv/pliego-actualizaciones
  file_server
  header /v1/* Cache-Control "no-cache"
  header /packages/* Cache-Control "public, max-age=31536000, immutable"
  log { output file /var/log/caddy/pliego.log { roll_keep 7 } }
}
```
Configuración de la app: compilar la versión oficial con `PLIEGO_UPDATE_URL=https://actualizaciones.ejemplo.es`.
Hasta entonces, las pruebas usan `pliego-release serve` y `PLIEGO_UPDATE_URL=http://127.0.0.1:8787`.

## 10. Prueba local hecha (2026-11-03, Linux de la sesión; repetir en acr12)
`keygen` → `publish 0.2.0` (paquete de 2 MB) → `verify` → `serve` en `127.0.0.1:8787` → `check --current 0.1.0
--download-to` (detecta 0.2.0, descarga con progreso y verifica SHA-256) → `check --current 0.2.0` ("al día").
18 pruebas automáticas en verde (`cargo test -p pliego-update -p pliego-release`).
