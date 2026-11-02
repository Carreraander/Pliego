# Pliego: guía para agentes

Pliego es una alternativa libre (AGPL-3.0-or-later) a Acrobat Pro para Windows: núcleo Rust sobre MuPDF,
interfaz Tauri 2 + WebView2 + SolidJS y CLI `pliego`. Idioma del proyecto: español (código e identificadores en inglés).

Antes de tocar nada, lee:
- `docs/CONVENTIONS.md`: reglas de código, ramas, PRs, licencias y qué archivos compartidos puedes tocar.
- `docs/ARCHITECTURE.md`: capas, crates y contratos.
- Tu flujo de trabajo en `docs/workstreams/` si te asignaron uno.

Comprobaciones antes de cada push (todas deben pasar):

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check licenses   # si tienes cargo-deny
```

Equipo de pruebas Windows: usuario `acr12` en `DESKTOP-41TJN0V`. Cada flujo trabaja en su propia carpeta
`C:\Users\acr12\dev\pliego-<flujo>` para no pisar a otros. Nunca borres carpetas de otros flujos.
