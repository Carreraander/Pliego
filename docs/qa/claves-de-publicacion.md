# Claves de publicación: copia de seguridad sin conexión

Pliego usa dos claves distintas. Ninguna secreta va al repositorio, al VPS ni a un chat.

| Clave | Para qué | Dónde vive |
| --- | --- | --- |
| Firma Ed25519 (`pliego-update.key`) | Firmar los manifiestos de actualización | Equipo de confianza (acr12: `D:\dev-cache\release-keys\`), secreto `PLIEGO_UPDATE_SIGNING_KEY` del entorno `release` de GitHub y **copia sin conexión** |
| SSH de despliegue (`pliego-deploy`) | Subir archivos por rsync al VPS | Secreto `UPDATE_SSH_KEY` del entorno `release`; solo la pública está en el VPS |

**Si se pierde la clave de firma y no hay copia, no se pueden publicar más actualizaciones**: las instalaciones solo
aceptan manifiestos firmados por las claves públicas que traen compiladas. Por eso hay una clave de reserva (§7 de
`docs/releasing.md`).

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
