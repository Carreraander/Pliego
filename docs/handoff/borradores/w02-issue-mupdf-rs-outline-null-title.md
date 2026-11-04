# Borrador de issue para messense/mupdf-rs (NO publicado)

**Título:** `Document::outlines()` hace `strlen(NULL)` (SEGV) cuando un marcador PDF no tiene `/Title`

**Cuerpo:**

### Qué pasa
En mupdf 0.8.0, `Document::walk_outlines` (`src/document.rs`, ~línea 238) hace

```rust
let title = CStr::from_ptr((*next).title).to_string_lossy().into_owned();
```

sin comprobar que `(*next).title` sea nulo. MuPDF deja `fz_outline.title` a NULL cuando una entrada de `/Outlines` no tiene `/Title`, así que un PDF cualquiera (o malicioso) hace que `Document::outlines()` aborte el proceso con SEGV en `strlen`.

### Reproducción
Hallado con cargo-fuzz. Un PDF mínimo cuyo `/Outlines` contiene un nodo sin `/Title`:

```rust
let doc = mupdf::Document::from_bytes(&std::fs::read("outline-null-title.pdf")?, "application/pdf")?;
let _ = doc.outlines(); // SIGSEGV
```

Traza (AddressSanitizer): `strlen` ← `CStr::from_ptr` ← `mupdf::document::Document::walk_outlines` (document.rs:238) ← `Document::outlines` (document.rs:272). Adjunto `outline-null-title.pdf` (1,3 KB, generado, sin datos personales).

### Arreglo propuesto
Tratar el título nulo como cadena vacía:

```rust
let title = if (*next).title.is_null() {
    String::new()
} else {
    CStr::from_ptr((*next).title).to_string_lossy().into_owned()
};
```

Conviene revisar igual `uri` (ya se comprueba) y cualquier otro `CStr::from_ptr` sobre campos de `fz_outline`/`fz_link` que MuPDF pueda dejar nulos. Puedo enviar el PR con un test si os parece bien.

Versión: mupdf 0.8.0 (mupdf-sys 0.8.0), Linux x86_64.
