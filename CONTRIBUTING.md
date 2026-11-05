# Contribuir a Pliego

Gracias por querer ayudar. Pliego se publica con la [Business Source License 1.1](LICENSE): el código es visible y usarlo
es gratis, pero vender Pliego, integrarlo en un producto de pago u ofrecerlo como servicio de pago requiere una licencia
comercial. Eso afecta a cómo se aceptan las contribuciones.

> **Estado:** el [acuerdo de contribución](CLA.md) es un borrador pendiente de revisión legal. Hasta que entre en vigor,
> abre una *issue* para proponer cambios; no se fusionan pull requests de terceros.

## Antes de tu primer pull request

1. Lee el [acuerdo de contribución (CLA)](CLA.md). Conservas la titularidad de tu código y concedes al titular del proyecto
   el derecho a relicenciarlo y sublicenciarlo (también con licencias comerciales).
2. Acéptalo con un comentario en tu primer pull request, con el texto exacto que indica la cláusula 10.
3. Si trabajas para una empresa o estudias en un centro que pueda tener derechos sobre lo que escribes, necesitas su
   autorización por escrito (cláusula 5).

## Reglas de código

- Idioma del proyecto: español (código e identificadores en inglés). Reglas completas en
  [`docs/CONVENTIONS.md`](docs/CONVENTIONS.md) y capas en [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).
- Antes de cada push, todo esto debe pasar:

  ```
  cargo fmt --all --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  cargo deny check licenses bans sources
  ```

- **No copies ni portes código de MuPDF, de Adobe ni de ningún programa GPL, LGPL o AGPL.** Escribe a partir de la
  especificación PDF y del comportamiento observable.
- **Dependencias nuevas:** solo con las licencias de [`deny.toml`](deny.toml) (permisivas). Si exigen aviso expreso, añádelas
  a [`THIRD-PARTY.md`](THIRD-PARTY.md).
- **Código generado con IA:** dilo en la descripción del pull request si una parte sustancial lo es, y revísalo línea a
  línea antes de enviarlo.
- Identidad propia: nada de nombres, iconos, textos o disposición que imiten a Acrobat o a otro producto.

## Vulnerabilidades

No las publiques en una *issue*: usa «Report a vulnerability» en la pestaña Security del repositorio.
