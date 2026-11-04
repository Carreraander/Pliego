# Propuesta para mupdf-rs: exponer el subconjunto de fuentes (`pdf_subset_fonts`)

(Borrador en inglés, para publicar como issue en el repositorio de `mupdf-rs`. Lo publica Ander si quiere.)

**Title:** Expose `pdf_subset_fonts` (font subsetting) on `PdfDocument`

**Body:**

We are building a PDF optimizer on top of `mupdf` 0.8 and would like to shrink embedded fonts by subsetting them,
as `mutool clean -S` does. The C function exists and `mupdf-sys` already generates its binding:

```c
void pdf_subset_fonts(fz_context *ctx, pdf_document *doc, int pages_len, const int *pages);
```

It cannot be called safely from a downstream crate today:

1. `PdfDocument::as_raw()` and `context()` are `pub(crate)`, so there is no way to reach the `pdf_document` pointer or
   the context.
2. The function can `fz_throw`, which longjmps; calling it directly from Rust is undefined behaviour. The crate's own
   `mupdf_*` C wrappers use `fz_try`/`fz_catch` for this reason.

Proposal: add a wrapper `mupdf_pdf_subset_fonts(ctx, doc, pages, len, &err)` in `mupdf-sys` and

```rust
impl PdfDocument {
    /// Subset the fonts used by the given pages (0-based). Experimental in MuPDF.
    pub fn subset_fonts(&mut self, pages: &[usize]) -> Result<(), Error>;
}
```

Notes: MuPDF documents the function as experimental; the effect only shows after saving (with garbage collection
enabled so that unused glyph data is dropped). We are happy to send a PR and a test (subset a PDF with an embedded
TrueType font and check the saved size shrinks and the text still extracts).
