# PDF cifrados de prueba (W40)

Generados con **qpdf 11.9** a partir de `tests/corpus/basic/hola.pdf` (dos páginas, título «Documento de prueba»,
dos marcadores) salvo los que indican `-image` (`tests/corpus/office/escaneado.pdf`) y `-fonts`
(`tests/corpus/office/articulo-1.pdf`). Son un productor distinto al que escribe el modelo propio, que es el
objetivo: `crates/pliego-pdf/tests/crypto.rs` los lee con lopdf y comprueba contraseñas, contenido, cadenas y permisos.

| Archivo | Cifrado | Usuario / propietario |
| --- | --- | --- |
| `r2-rc4-40` | RC4 40 bits (R2) | `u2` / `o2` |
| `r3-rc4-128`, `r3-fonts` | RC4 128 bits (R3) | `u3` / `o3` |
| `r4-rc4-128`, `r4-objstm` | RC4 con filtros de cifrado (V4, R4) | `u4` / `o4` |
| `r4-aes-128`, `r4-cleartext-metadata`, `r4-fonts` | AES-128 (AESV2, R4) | `u4` / `o4` |
| `r4-latin1` | AES-128, contraseñas con caracteres Latin-1 | `contraseña` / `propietaria` |
| `r5-aes-256` | AES-256 R5 (extensión de Adobe, en desuso) | `u5` / `o5` |
| `r6-aes-256`, `r6-image`, `r6-objstm`, `r6-cleartext-metadata` | AES-256 R6 (PDF 2.0) | `u6` / `o6` |
| `r6-unicode` | AES-256 R6, contraseñas con `ñ`, `€` (SASLprep) | `ñandú€` / `dueño` |
| `r6-empty-user` | AES-256 R6, contraseña de usuario vacía | (vacía) / `o6` |
| `r6-restricted` | AES-256 R6 sin imprimir, copiar, modificar, anotar, rellenar ni ensamblar | `u6` / `o6` |
| `direct-r6-xrefstream`, `direct-r4-xrefstream` | Los de `objstm`, con el diccionario `/Encrypt` **directo** en el flujo de referencias | `u6` / `u4` |

`r*-objstm` llevan flujos de objetos y de referencias (`--object-streams=generate`). Los `direct-*` se hicieron
sustituyendo `/Encrypt N 0 R` por el diccionario en el flujo de referencias (qpdf siempre lo escribe indirecto; otros
productores no).
