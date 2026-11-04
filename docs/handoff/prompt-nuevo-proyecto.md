# Arrancar el proyecto en la cuenta nueva

## 1. Lo que Ander hace a mano en la cuenta nueva (antes de pegar el prompt)

1. **Crear el proyecto** en Claude (por ejemplo «AcrobatFree») y, en sus ajustes, **añadir el repositorio**
   `https://github.com/Carreraander/Pliego`.
2. **Conectar GitHub** con la cuenta nueva (https://claude.ai/connect-github) e instalar o autorizar la app de Claude
   en el repo `Carreraander/Pliego` (https://github.com/apps/claude/installations/select_target) para que los hilos
   puedan crear ramas, abrir PRs y fusionarlos.
3. **En acr12**, iniciar sesión en Claude Desktop con la cuenta nueva, activar Remote Control para ese equipo y
   **aprobar la carpeta** `C:\Users\acr12\AcrobatFree` (y `D:\dev` si la app lo ofrece). Comprobar que
   `C:\Users\acr12\.claude\settings.json` sigue teniendo `PowerShell(npm:*)`, `PowerShell(npx:*)` y
   `PowerShell(cargo:*)`.
4. **Poner las instrucciones del proyecto** (ajustes del proyecto), con este texto:

   > Proyecto Pliego: alternativa libre (AGPL-3.0 + MuPDF) a Acrobat Pro para Windows. Repo Carreraander/Pliego. Habla
   > en español. El estado, las decisiones y las reglas están en `docs/handoff/` del repo: léelo antes de actuar y
   > mantenlo al día. Gestión y diseño con Opus 5.5; implementación con Sonnet 5.5, un hilo por flujo. Trabaja con
   > autonomía: pregúntame solo permisos reales y cambios de diseño, licencia o alcance. PRs con squash, ramas
   > actualizadas con merge de main, nunca rebase ni force-push; gestión fusiona con CI verde. En acr12: todo en D:,
   > `. .\tools\acr12-env.ps1` en cada terminal y todo lo pesado con `tools\acr12-turno.ps1` (un solo turno).

5. Si la cuenta nueva no tiene Opus 5.5 y Sonnet 5.5, avisar al coordinador de qué modelos hay.

## 2. Prompt para pegar en el chat del proyecto nuevo

```text
Retomamos el proyecto Pliego, que viene de otra cuenta. Quiero la misma coordinación y gestión que había allí.

Pliego es una alternativa libre (AGPL-3.0 + MuPDF) a Adobe Acrobat Pro para Windows, en el repo
github.com/Carreraander/Pliego. Todo el estado del proyecto anterior está en el repo, en docs/handoff/: no hay
hilos, memoria ni carpeta compartida de antes. El equipo de pruebas es acr12 (DESKTOP-41TJN0V, usuario acr12).

Haz esto en orden:

1. Lee en el repo, enteros: CLAUDE.md, docs/handoff/README.md, docs/handoff/reglas-operativas.md,
   docs/handoff/memoria.md, docs/handoff/lecciones-tecnicas.md, docs/CONVENTIONS.md, docs/ARCHITECTURE.md,
   docs/workstreams/README.md y la lista de docs/handoff/flujos/.

2. Reconstruye la memoria del proyecto a partir de docs/handoff/memoria.md: un archivo por tema (proyecto,
   preferencias de Ander, reglas de fusión y ramas, acr12, diseño de la app, pendientes de Ander e ids de hilos).
   Usa la fecha de hoy y enlaza los archivos del repo en vez de copiarlos.

3. Comprueba la configuración: que el repo está conectado y se puede escribir en él (lista los PRs abiertos; deben
   estar #93 y #96 salvo que alguien los haya fusionado), y que el dispositivo acr12 está disponible con la carpeta
   C:\Users\acr12\AcrobatFree aprobada. Fija acr12 como dispositivo por defecto del proyecto. Si falta algo, dime
   exactamente qué tengo que hacer y sigue con lo demás.

4. Abre un hilo de gestión con Opus 5.5, titulado "Gestionar el desarrollo de Pliego". Su trabajo: revisar y
   fusionar PRs con CI verde (squash; nunca rebase ni force-push), mantener las piezas compartidas
   (pliego-core, pliego-engine-mupdf, CI, tools/acr12-*), repartir el turno único de acr12, resolver dudas de los
   flujos, hacer de dueño del diseño (docs/design/) y mantener docs/handoff/ al día. Primeras tareas: fusionar
   #96 (W12) tras la verificación con veraPDF y #93 (W10) tras los benchmarks en acr12, y llevar la cuenta de lo
   que falta de cada flujo.

5. Abre un hilo de implementación con Sonnet 5.5 por cada flujo con trabajo pendiente según
   docs/handoff/README.md: W01, W02, W03, W04, W05, W06, W07, W08, W09, W10, W12, W13, W14, W15, W16, W17, W18, W19,
   W20, W21 y W22. W11 no necesita hilo: solo falta que yo revise sus PDFs en Acrobat Reader; lo cierra gestión
   cuando te lo confirme. A cada hilo dale como instrucciones: "Eres el flujo WNN de Pliego. Lee
   docs/workstreams/<archivo del flujo>, docs/handoff/flujos/wNN.md, docs/handoff/reglas-operativas.md y
   docs/CONVENTIONS.md. Haz lo que falta para cerrar el flujo. Avisa al hilo de gestión con el enlace de cada PR;
   gestión los fusiona. Actualiza docs/handoff/flujos/wNN.md en tu PR." Prioridad: primero W03 (tenía 4 arreglos
   de acr12 sin PR), W05 (firma con el almacén de Windows), W22 (integración en la app), W10 y W12 (PRs abiertos)
   y W01.

6. Reglas de acr12 para todos los hilos (están en reglas-operativas.md): compilar e instalar en D:, cada flujo en
   su propio clon D:\dev\pliego-<flujo>, cargar `. .\tools\acr12-env.ps1` en cada terminal, todo lo pesado con
   `.\tools\acr12-turno.ps1 -Flujo wNN -- <orden>` (un solo turno, cola por orden de llegada, CARGO_BUILD_JOBS=4),
   `tauri dev` y servidores fuera del turno, cerrar los procesos propios al acabar y nunca tocar carpetas ni
   procesos de otros. acr12 tiene 16 GB: no lances muchas sesiones de Remote Control a la vez; que gestión las
   escalone.

7. Trabaja con autonomía: no me consultes salvo permisos reales (aprobar un diálogo en acr12, cerrar procesos
   ajenos) y cambios de diseño, licencia o alcance. Mis pendientes están en docs/handoff/README.md
   ("Pendiente de Ander"): recuérdamelos una vez, juntos, en un mensaje corto.

Cuando esté todo en marcha, dame un resumen corto de lo que has abierto y de lo que necesitas de mí.
```

## 3. Después del arranque

- El coordinador nuevo debe guardar en memoria los ids de los hilos que abra (sustituyen a los de la cuenta antigua,
  que ya no sirven).
- Las sesiones de Remote Control abiertas desde la cuenta antigua en acr12 deben cerrarse; antes, comprobar que no
  quedan cambios sin subir en los clones `pliego-w03` y `pliego-w05` (arreglos de anotaciones y `WindowsStoreSigner`).
