# Pliego: guía para agentes

Pliego es una alternativa gratuita y con el código a la vista (BUSL-1.1) a Acrobat Pro para Windows: núcleo Rust sobre lopdf + PDFium,
interfaz Tauri 2 + WebView2 + SolidJS y CLI `pliego`. Idioma del proyecto: español (código e identificadores en inglés).

Antes de tocar nada, lee:
- `docs/CONVENTIONS.md`: reglas de código, ramas, PRs, licencias y qué archivos compartidos puedes tocar.
- `docs/ARCHITECTURE.md`: capas, crates y contratos.
- Tu flujo de trabajo en `docs/workstreams/` si te asignaron uno, y su estado en `docs/handoff/flujos/`.
- `docs/handoff/`: estado global, decisiones y reglas operativas (acr12, turnos, fusión de PRs).

Comprobaciones antes de cada push (todas deben pasar):

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check licenses   # si tienes cargo-deny
```

Equipo de pruebas Windows: usuario `acr12` en `DESKTOP-41TJN0V`. Cada flujo trabaja en su propia carpeta
`D:\dev\pliego-<flujo>` (los clones antiguos siguen en `C:\Users\acr12\dev\pliego-<flujo>`) para no pisar a otros.
Antes de compilar, `. .\tools\acr12-env.ps1`; todo lo pesado con `.\tools\acr12-turno.ps1 -Flujo wNN -- <orden>`. Nunca borres carpetas de otros flujos.
