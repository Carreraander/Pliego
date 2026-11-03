# Equipo de pruebas acr12

Equipo Windows de pruebas del proyecto: `DESKTOP-41TJN0V`, usuario `acr12`. Cada flujo trabaja en `C:\Users\acr12\dev\pliego-<flujo>`.

## Preparación

```powershell
cd C:\Users\acr12\dev\pliego-<flujo>
powershell -ExecutionPolicy Bypass -File tools\setup-acr12.ps1 -Report acr12-setup.md
```

El script es idempotente (instala lo que falte con winget y cargo; `-CheckOnly` solo verifica) y devuelve código 1 si falta algo.

## Primera ejecución (2026-11-03)

- Windows: Microsoft Windows 10 Pro 10.0.19045
- `CARGO_HOME`: `D:\dev-cache\cargo-home` (el script lo respeta)

| Herramienta | Estado | Versión |
|---|---|---|
| Git | ya estaba | 2.53.0.windows.1 |
| Node LTS | ya estaba | v24.16.0 |
| LLVM (clang) | ya estaba | 23.1.4 |
| WiX Toolset | instalado | 3.14.1.8722 (activó la característica de Windows NetFx3) |
| VS Build Tools C++ | ya estaba | VS 17.13.35919.96 |
| Rust (rustc / cargo) | ya estaba | 1.96.0 |
| cargo-deny | instalado (compilación ~6 min) | 0.20.2 |

## Resultados de comprobaciones y rendimiento

Pendiente: segunda pasada de idempotencia, `cargo fmt/clippy/test` en Windows y benchmarks por versión.
