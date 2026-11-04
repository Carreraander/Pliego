# Instalador beta de Pliego

Cómo montar `Pliego-Setup-<versión>-beta.exe` en acr12 con una sola orden y una actualización de prueba firmada.
Responsable: flujo W21 (`tools/build-beta.ps1`). La versión oficial se publica con `docs/releasing.md`.

## Qué es y qué no es
- Es un instalador **de prueba**. **Confía en producción + prueba**: acepta manifiestos firmados con las claves de
  producción del repo (`BUILTIN_RELEASE_KEYS` en `crates/pliego-update/src/keys.rs`, #129) **y** con la clave de
  **PRUEBA** (`D:\dev-cache\beta-keys\pliego-PRUEBA-beta.key`), que se añade solo en esta compilación con
  `PLIEGO_UPDATE_PUBKEYS` para firmar el 0.1.1 del E2E (`release_keys()` admite ambas). La de producción no tiene
  nada que ver con la de prueba (`docs/qa/claves-de-publicacion.md`). La clave de prueba **no** va al repositorio ni
  a ningún servidor; el script se niega a usar una carpeta dentro del repo.
- Sin Authenticode (aparecerá SmartScreen). La app comprueba actualizaciones solo contra `http://127.0.0.1:8787`
  (servidor local de `pliego-release serve`) salvo que se cambie `-UpdateUrl` o `PLIEGO_UPDATE_URL` en ejecución.

## Uso (acr12, un solo turno)
```powershell
cd D:\dev\pliego-w21        # o el clon desde el que se lance, con main actualizado
. .\tools\acr12-env.ps1
.\tools\acr12-turno.ps1 -Flujo w21 -- powershell -NoProfile -File tools\build-beta.ps1
```
Opciones: `-Version`, `-UpdateVersion` (0.1.1), `-OutDir` (`D:\dev\pliego-beta`), `-KeyDir`, `-UpdateUrl`,
`-SkipUpdate`, `-SkipCargo`.

## Qué hace
1. Crea la clave de prueba si no existe (`pliego-release keygen --name pliego-PRUEBA-beta`) y compila su pública con
   `PLIEGO_UPDATE_PUBKEYS` en la app y en `pliego-setup` (el setup elevado la usa para `--update --manifest`).
2. `installer\setup\build.ps1`: `tauri build` de la app, release del worker, la CLI, el lanzador y `pliego-setup`
   (= `Uninstall.exe`); empaqueta el `.paq` y pega el `.paq` al instalador.
3. Copia el instalador como `Pliego-Setup-<versión>-beta.exe` más su `.sha256`.
4. Genera un `.paq` de la versión de prueba (misma app, solo cambia la versión del paquete) y lo publica firmado con
   `pliego-release publish` en `D:\dev\pliego-beta\actualizaciones`, y lo verifica con `pliego-release verify`.

Salida en `D:\dev\pliego-beta\`: `Pliego-Setup-0.1.0-beta.exe`, `.sha256`, `actualizaciones\` (árbol listo para servir)
y `dist\` (`.paq`, sumas, carpeta de montaje).

## E2E de autoactualización (W22)
```powershell
D:\dev-cache\target\<clon>\release\pliego-release.exe serve --root D:\dev\pliego-beta\actualizaciones
# fuera del turno; instalar el beta y abrir Pliego: ofrece 0.1.1 (la app instalada se queda en 0.1.0 hasta aplicar).
```
Tras aplicar, el lanzador activa `pending` (por usuario) o, por equipo, pide UAC y ejecuta
`Uninstall.exe --update --paq … --manifest …`. Cerrar `serve` al terminar.

## Limitaciones conocidas
- La app de 0.1.1 es el mismo binario que 0.1.0: el número que muestra la app no cambia (sí el del paquete, el
  registro de Windows tras `post_update` y la carpeta `app-0.1.1`). Para probar con un binario distinto, reconstruir con
  otra versión del workspace.
- El script usa `powershell.exe` 5.1: por eso `build-beta.ps1` se guarda en UTF-8 **con BOM** (sin él, 5.1 lo lee
  como ANSI y las notas «Versión…» llegan estropeadas al manifiesto firmado).
- Volver a lanzarlo republica 0.1.1 sobre el árbol existente; para empezar de cero, borrar antes
  `D:\dev\pliego-beta\actualizaciones`.
- Validado en acr12 el 2026-11-04 (compilación completa, `publish` y `verify` correctos).
