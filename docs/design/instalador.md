# Pliego · Instalador (`Pliego-Setup-x.y.z-x86_64.exe`)

Estado: propuesta v1 (2026-11-03). Pantallas en `docs/design/prototipo.html` (sección Instalador).

## 1. Decisión

Instalador propio, **`pliego-setup`**, escrito en Rust, con interfaz web (WebView2 vía `wry`/`tao`, MIT/Apache) que
comparte tokens con la app. Un único `.exe` que contiene el paquete de la app comprimido.

| Opción | Aspecto | Actualizaciones | Por qué no / por qué sí |
| --- | --- | --- | --- |
| NSIS de Tauri con plantilla | Asistente clásico de Windows; se puede poner imagen lateral y de cabecera | Plugin de Tauri | No llega a "bonito": controles Win32 de los 2000, sin tema oscuro ni animación |
| WiX Burn con interfaz WPF | Bueno, pero .NET y XAML aparte | Ninguna integrada | Otra pila (C#), pesado, sin compartir diseño |
| Velopack | Una pantalla de bienvenida (imagen) y listo | Muy buenas (deltas, lanzador) | Sin opciones ni pasos; manifiesto sin firma propia; dependemos de su formato |
| **Propio (`pliego-setup`)** | Lo que diseñemos, con los mismos tokens que la app, claro/oscuro, animado | Integradas con `pliego-update` (mismo paquete, misma firma) | Más código (~2–3 k líneas), pero controlado y pequeño |

El MSI de WiX (W10) se mantiene **para empresas** (GPO/Intune, instalación por máquina, sin autoactualización) y el
ZIP portátil para quien no quiera instalar.

## 2. Estructura en disco

Instalación por usuario por defecto, sin UAC (como los navegadores y editores modernos):

```
%LOCALAPPDATA%\Programs\Pliego\
  Pliego.exe            lanzador (~300 KB): lee `current`, ejecuta app-<v>\pliego-app.exe con los mismos argumentos
  bin\pliego.exe        lanzador de la CLI (a app-<v>\pliego.exe); `bin\` es lo que se añade al PATH
                        (no puede estar junto a Pliego.exe: NTFS no distingue mayúsculas)
  Uninstall.exe         copia de pliego-setup.exe (desinstalar y tareas posteriores a actualizar)
  current               texto: "0.4.0"
  pending               (solo si hay actualización preparada) texto: "0.5.0"
  app-0.4.0\            versión actual: pliego-app.exe, pliego.exe, pliego-worker.exe, recursos, files.json
  app-0.3.2\            versión anterior, para volver atrás si la nueva no arranca
```

"Para todos los usuarios" (opción, pide UAC): misma estructura en `C:\Program Files\Pliego\` y registro en HKLM.

Por qué el lanzador: la actualización se prepara en `app-<nueva>\` **mientras Pliego está abierto** y el cambio es
reescribir un archivo de texto de forma atómica. Sin pantallas, sin esperar, y volver atrás es trivial. Los accesos
directos, la asociación de .pdf y el menú contextual apuntan a `Pliego.exe`, que no cambia de ruta.
El lanzador fija el `AppUserModelID` `Pliego.App` para que la barra de tareas agrupe bien; los accesos directos
llevan el mismo.

## 3. Paquete de la app (`.paq`)

Mismo archivo para el instalador y para el actualizador:
- `tar` comprimido con zstd (nivel 19, `--long`) de la carpeta `app-<v>\`.
- Dentro, `files.json`: lista de archivos con tamaño y SHA-256, y versión. Tras extraer se verifica cada archivo.
- El instalador lleva el `.paq` pegado al final del ejecutable con un pie de 64 bytes
  (`PLIEGOPAQ1`, desplazamiento, longitud, SHA-256). `pliego-setup --pack` produce el `.exe` a partir del binario
  base y el `.paq`.
- Objetivo de tamaño: instalador < 40 MB (el `.paq` de la app ronda 25–30 MB).

## 4. Pantallas (ventana 680 × 440, sin marco, esquinas de 12 px, Mica, arrastrable)

1. **Bienvenida.** Logo animado (la hoja se dobla), "Pliego", una línea: "Lee, firma y organiza tus PDF. Libre y sin
   nube.". Botón principal **Instalar** y enlace "Opciones de instalación". Pie: "Software libre bajo AGPL-3.0 ·
   Ver licencia" (abre la licencia en un panel, no hace falta aceptar nada: la AGPL no exige aceptación para usar).
2. **Opciones** (solo si se pide): carpeta (con "Cambiar…"), Para mí / Para todos los usuarios (escudo UAC),
   Acceso directo en el escritorio, Añadir al menú Inicio (siempre), Abrir PDF con Pliego, Opciones en el menú
   contextual del Explorador, Añadir `pliego` a la línea de órdenes (PATH), Buscar actualizaciones automáticamente.
   Botones "Atrás" e "Instalar".
3. **Progreso.** La hoja del logo se va doblando en pliegos según avanza; barra de progreso fina y texto del paso
   ("Copiando archivos", "Registrando Pliego en Windows", "Creando accesos directos"). Normalmente < 10 s.
4. **Listo.** "Pliego está instalado." Botón **Abrir Pliego**. Casilla "Usar Pliego como visor de PDF
   predeterminado" (abre Configuración de Windows en la página de Pliego, ver §6). Tres consejos breves
   (Ctrl+K, soltar archivos, firmar con DNIe).

Otras pantallas:
- **Pliego ya está instalado**: misma versión → "Reparar" / "Desinstalar"; versión anterior → "Actualizar a 0.5.0"
  (conserva ajustes); versión posterior → "Ya tienes una versión más reciente (0.6.0)" y solo "Abrir Pliego".
- **Pliego está abierto**: "Cierra Pliego para continuar. [Cerrar Pliego por mí]" (pide a la app que se cierre por
  su canal de instancia única, que ofrece guardar).
- **Desinstalar**: "¿Desinstalar Pliego?" con casilla "Conservar mis ajustes y firmas guardadas" (marcada) →
  progreso → "Pliego se ha desinstalado" con enlace opcional "Cuéntanos por qué (abre una página web)".
- **Errores**: sin espacio ("Faltan 120 MB libres en C:"), paquete dañado ("El instalador está dañado. Descárgalo de
  nuevo"), sin permisos, versión de Windows anterior a 10 2004. Siempre con "Copiar detalles" y la ruta del registro.

Tema claro/oscuro según Windows; idioma según Windows (es/en). Accesible con teclado y Narrador.

## 5. Sin WebView2

Windows 11 y Windows 10 actualizado traen WebView2. Si falta (se mira la clave
`HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}` y su equivalente
en HKCU), el instalador muestra un `TaskDialog` nativo con barra de progreso "Instalando un componente de Windows
necesario (WebView2)…", descarga el instalador oficial de Microsoft (Evergreen Bootstrapper, ~2 MB) y lo ejecuta en
silencio. Después sigue con la interfaz normal. Si no hay red: mensaje con el enlace de descarga.
No se redistribuye nada de Microsoft dentro del instalador.

## 6. Registro en Windows

Por usuario (HKCU; por máquina, HKLM):
- Desinstalación: `Software\Microsoft\Windows\CurrentVersion\Uninstall\Pliego` (`DisplayName`, `DisplayVersion`,
  `Publisher` = "Proyecto Pliego", `DisplayIcon`, `InstallLocation`, `UninstallString`, `QuietUninstallString`,
  `EstimatedSize`, `NoModify`=1, `URLInfoAbout`).
- Tipo de archivo: ProgID `Pliego.Document` (icono, verbo `open` → `"…\Pliego.exe" "%1"`), `.pdf\OpenWithProgids`,
  `Software\Clients\…` no; sí `Software\RegisteredApplications` + `Software\Pliego\Capabilities` (nombre, descripción,
  `FileAssociations\.pdf = Pliego.Document`) para aparecer en Aplicaciones predeterminadas.
- Windows 10/11 no dejan fijar la app predeterminada por código: la casilla abre
  `ms-settings:defaultapps?registeredAppUser=Pliego` (Windows 11) o `ms-settings:defaultapps` (Windows 10).
  Pliego no vuelve a preguntarlo salvo desde Ajustes > General.
- Menú contextual del Explorador y vista previa: los registra `pliego-shell` (W10). El instalador llama a su
  registro y desregistro.
- Tras cambiar asociaciones: `SHChangeNotify(SHCNE_ASSOCCHANGED)`.
- PATH (opcional): añade `%LOCALAPPDATA%\Programs\Pliego\bin` al PATH del usuario y difunde `WM_SETTINGCHANGE`.

## 7. Línea de órdenes del instalador

```
Pliego-Setup.exe                     interfaz normal
Pliego-Setup.exe /S | --silent       sin interfaz, opciones por defecto
  --per-machine                      para todos los usuarios (requiere ejecutarse elevado)
  --dir <ruta>                       carpeta
  --no-desktop-shortcut --no-assoc --no-context-menu --no-path --no-updates
  --log <archivo>                    registro (por defecto %TEMP%\pliego-setup.log)
Uninstall.exe --uninstall [--silent] [--purge]   --purge borra también ajustes
Uninstall.exe --post-update          tareas tras cambiar de versión (lo llama el lanzador, ver actualizaciones.md)
pliego-setup --pack --base <exe> --paq <archivo> --out <exe>   (solo compilación)
```
Códigos de salida: 0 bien, 1602 cancelado por el usuario, 1603 error, 3010 hace falta reiniciar (no se espera).

## 8. Desinstalar
1. Si Pliego está abierto, pedir que se cierre.
2. Desregistrar `pliego-shell`, asociaciones, entrada de desinstalación, PATH, accesos directos.
3. Borrar la carpeta de instalación. Como `Uninstall.exe` está dentro, se copia a `%TEMP%` y se relanza con
   `--cleanup-from <ruta>`, que espera a que termine el proceso original y borra el resto.
4. Ajustes en `%APPDATA%\Pliego` y caché en `%LOCALAPPDATA%\Pliego`: se conservan salvo que se desmarque la casilla
   (o `--purge`). La caché siempre se borra.

## 9. Firma de código
- Sin firma, SmartScreen avisa ("Windows protegió su PC") hasta que el ejecutable gane reputación.
- Recomendado: **SignPath Foundation** (firma gratuita para proyectos de código abierto, desde GitHub Actions) o
  **Azure Trusted Signing** (de pago, barato). Se firman `Pliego-Setup.exe`, `Pliego.exe`, `pliego-app.exe`,
  `pliego.exe`, `pliego-worker.exe` y la DLL de `pliego-shell`.
- El script de compilación ya deja el paso preparado (`SIGN_TOOL`/`SIGN_ARGS` vacíos = sin firmar).
- La firma Authenticode es independiente de la firma Ed25519 de las actualizaciones: las dos se comprueban.

## 10. Pruebas (en acr12)
- Instalar, abrir, desinstalar limpio (sin restos en registro ni disco salvo ajustes conservados).
- Por usuario y por máquina; con y sin WebView2 (máquina virtual limpia si es posible).
- Reinstalar encima (reparar), actualizar desde la versión anterior, intentar bajar de versión.
- Silencioso (`/S`) y códigos de salida.
- Narrador y solo teclado en todas las pantallas; 100 %, 150 % y 200 % de escala.
- Tamaño final < 40 MB; tiempo de instalación < 10 s en acr12.
