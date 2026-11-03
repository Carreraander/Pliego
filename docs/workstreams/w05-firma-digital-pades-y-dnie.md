# W05 · Firma digital PAdES y DNIe

Ola 1. Depende de: base existente; coordinar con W02 para guardado incremental.

## Contexto (léelo entero)
Eres una sesión de implementación del proyecto **Pliego**: una alternativa libre (AGPL-3.0-or-later) y gratuita a
Adobe Acrobat Pro para Windows. Ander es el dueño del proyecto. Hay un hilo de gestión ("Diseñar alternativa open
source a Acrobat Pro", id `cmsg_01VzNAbDDgN8F4HKcyc76pSD2RqHEmBxL1s12gPvzmcf2G`) que coordina todos los flujos, revisa
PRs y decide sobre interfaces compartidas. Escríbele con `message_thread` usando ese id.

- Repositorio: https://github.com/Carreraander/Pliego (rama `main`). Añádelo a tu sesión y clónalo.
- Diseño completo: https://claude.ai/artifact/Wu4ZXyGqToREgugipQuNgw
- Lee antes de empezar: `CLAUDE.md`, `docs/CONVENTIONS.md`, `docs/ARCHITECTURE.md`.
- Stack: Rust (edición 2024) + MuPDF vía crate `mupdf` 0.8 (AGPL) · Tauri 2 + WebView2 + SolidJS · CLI `pliego`.
- Ya existe: `pliego-core` (Error, PageRange, JobContext, Report, trait PdfEngine), `pliego-engine-mupdf`
  (render de página/tesela, texto, búsqueda, marcadores) y `pliego-cli` (info, render, text, search).
- Equipo de pruebas Windows: usuario `acr12` en `DESKTOP-41TJN0V`. Para compilar o probar allí, pide una sesión
  Remote Control en ese equipo. Trabaja SOLO en `C:\Users\acr12\dev\pliego-w05` (clon propio). Ander autorizó
  instalar las herramientas de desarrollo necesarias; si ya están (Rust, VS Build Tools, Node, Git, LLVM), reutilízalas.
  No borres ni modifiques carpetas de otros flujos.
- Producto propio: nada de nombres, iconos, textos o disposición que imiten a Acrobat.

## Reglas de trabajo
1. Solo editas las rutas que son tuyas (abajo) y, con cambios mínimos, los archivos compartidos de la CLI que lista
   `docs/CONVENTIONS.md`. Si necesitas cambiar `pliego-core` u otra ruta ajena, pídelo al hilo de gestión.
2. Rama `w05-<tema>` desde `main`. PRs pequeños e incrementales (< 800 líneas), cada uno útil por sí solo.
   Para actualizar la rama, haz merge de `main` en ella (nunca rebase ni force-push); regenera `Cargo.lock`.
3. Antes de cada push: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace`. Dependencias nuevas solo con licencias de `deny.toml`.
4. Al abrir cada PR, envía el enlace al hilo de gestión. Sigue la CI y las revisiones hasta que se fusione.
5. Si una decisión cambia el alcance o una interfaz compartida, pregunta al hilo de gestión antes de seguir.
6. Responde a Ander en español.

## Tus rutas
`crates/pliego-sign`, `crates/pliego-cli/src/commands/sign.rs`, `tests/corpus/sign/`.

## Objetivo
Firmar con certificado (archivo PKCS#12, almacén de Windows, tarjeta inteligente como DNIe o FNMT) y validar firmas, con resultados que Acrobat Reader acepte como válidos.

## Alcance
- Firma PAdES (ETSI EN 319 142) niveles B-B, B-T (sello de tiempo RFC 3161), B-LT (DSS con certificados, OCSP y CRL) y B-LTA (sello de documento). Firma invisible y visible (apariencia con nombre, fecha, motivo, imagen opcional). Firma de certificación (DocMDP) y firmas sucesivas con guardado incremental.
- Claves: PKCS#12 (.pfx/.p12) y, en Windows, almacén de certificados del usuario y tarjetas inteligentes vía CNG/NCrypt (crate `windows`); la clave privada nunca sale del proveedor. Diseño con trait `Signer` para soportar ambos.
- Criptografía con RustCrypto (`cms`, `x509-cert`, `der`, `sha2`, `rsa`, `p256`, `p384`) y cliente HTTP ligero para TSA/OCSP/CRL (`ureq` con rustls).
- Validación: integridad (ByteRange y digest), cadena hasta raíces de confianza (almacén de Windows + lista configurable, incluida la TSL española si es viable), revocación, sello de tiempo, detección de cambios posteriores permitidos/no permitidos. Informe legible.
- Campos de firma vacíos: crear y firmar uno existente.

## Interfaces que debes exponer
- `pliego_sign::{sign(input, out, &dyn Signer, SignOptions, ctx), validate(input) -> Vec<SignatureReport>, add_signature_field}`.
- `trait Signer { fn certificate_chain(&self) -> Vec<Certificate>; fn sign_digest(&self, alg, digest) -> Result<Vec<u8>>; }` con `Pkcs12Signer` y `WindowsStoreSigner` (`#[cfg(windows)]`, en `src/platform/windows.rs` con la excepción de `unsafe` documentada).
- `list_windows_certificates()` para que la app muestre el selector.
- CLI: `pliego sign --p12 archivo.p12 | --store <huella>`, `--level b-b|b-t|b-lt|b-lta`, `--tsa <url>`, `--visible pagina,x,y,w,h`; `pliego verify archivo.pdf`.

## Criterios de "hecho"
- Firmas B-B, B-T y B-LT validadas como correctas por pyHanko (`pyhanko sign validate`) en CI y por Acrobat Reader gratuito en acr12 (captura en el PR).
- Certificados de prueba generados en los tests (CA propia), nunca certificados reales en el repo.
- Firma con el almacén de Windows probada en acr12 con un certificado de prueba importado; con DNIe/FNMT si hay lector y certificado disponible (pregunta a Ander por el hilo de gestión).
- Modificar un byte tras firmar hace que `validate` informe de firma rota.
