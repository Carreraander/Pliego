# Claves de publicación: copia de seguridad sin conexión

Pliego usa dos claves distintas. Ninguna secreta va al repositorio, al VPS ni a un chat.

| Clave | Para qué | Dónde vive |
| --- | --- | --- |
| Firma Ed25519 (`pliego-update.key`) | Firmar los manifiestos de actualización | Solo en el equipo de firma (acr12: `D:\dev-cache\release-keys\`) y en la **copia sin conexión**. **Nunca** como secreto de GitHub |
| SSH de despliegue (`pliego-deploy`) | Subir archivos por rsync al VPS | Secreto `UPDATE_SSH_KEY` del entorno `release`; solo la pública está en el VPS |

**Si se pierde la clave de firma y no hay copia, no se pueden publicar más actualizaciones**: las instalaciones solo
aceptan manifiestos firmados por las claves públicas que traen compiladas. Por eso hay una clave de reserva (§7 de
`docs/releasing.md`).

Motivo: si la clave de firma estuviera en la CI, un flujo o una dependencia comprometidos podrían firmar una
actualización maliciosa, y esa clave es la única barrera. Subir archivos (la clave SSH) no da poder para firmar.

## Claves de producción (generadas en acr12 el 2026-11-04)

| Papel | Archivo | Huella (`key_id`) | Clave pública |
| --- | --- | --- | --- |
| Activa | `pliego-update.key` | `ab6fa8bae6005ac0` | `ed25519:mWq30UarbH3bZHlR4NEJafc5NVkq+X3bmFdSZn93+XQ=` |
| Reserva | `pliego-reserva.key` | `9d886dc4f92cdbde` | `ed25519:H8pykJe7e8W2PnOonRlJ8d2zcuTEy/Fj9R3axSp/XJY=` |

Las dos públicas están en `BUILTIN_RELEASE_KEYS` (`crates/pliego-update/src/keys.rs`). Las privadas están en
`D:\dev-cache\release-keys\` de acr12 (solo el usuario acr12). **Ander tiene que copiar esa carpeta a una USB cifrada
sin conexión** (pasos abajo); la de reserva no debería volver a conectarse a ningún equipo salvo para rotar.

## Copia de seguridad (Ander, una sola vez por clave)
1. En el equipo donde se generó, abrir PowerShell y comprobar que existe `pliego-update.key` y `pliego-update.pub`.
2. Copiar `pliego-update.key` (y la de reserva) a una **memoria USB cifrada** (BitLocker To Go: clic derecho en la
   unidad > Activar BitLocker, con contraseña larga). No usar nube, correo ni mensajería.
3. Guardar la contraseña del cifrado en el gestor de contraseñas, **no** en la misma USB.
4. Hacer una segunda USB idéntica y guardarla en otro lugar físico.
5. Comprobar la copia: `pliego-release verify --pubkey pliego-update.pub` solo valida la pública; para la privada,
   firmar un manifiesto de prueba con la copia (`pliego-release publish` a una carpeta temporal) y verificarlo.
6. En el equipo de trabajo, dejar el archivo en una carpeta con permisos solo para el usuario (en Windows:
   propiedades > Seguridad > Opciones avanzadas > desactivar herencia y quitar el resto de usuarios).

La clave **pública** sí se puede copiar libremente: va en `PLIEGO_UPDATE_PUBKEYS` o `BUILTIN_RELEASE_KEYS`.
