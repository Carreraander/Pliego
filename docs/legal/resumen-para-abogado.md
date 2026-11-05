# Pliego: resumen para la revisión legal

Versión 1 (2026-11-05). Preparado para el abogado de propiedad intelectual (España/UE) que revise el cambio de licencia.
**No es asesoramiento legal**: describe lo que hay, lo que propone el borrador y las preguntas abiertas. Todo lo marcado
como `[PENDIENTE]` en `LICENSE` y `CLA.md` lo rellena Ander con tu consejo. El PR que lo trae está en borrador y no se
fusiona hasta que acabe esta revisión.

## 1. Qué es Pliego y qué quiere Ander

Pliego es una alternativa a Acrobat Pro para Windows: núcleo en Rust (PDFium para dibujar y extraer texto, lopdf para
la estructura del PDF, Boa para los JavaScript de los formularios, Tesseract para OCR), interfaz de escritorio con Tauri
y CLI `pliego`. Unas 100.000 líneas de Rust y 14.000 de TypeScript.

Lo que quiere Ander (decidido el 2026-11-04):

- Que **cualquiera lo use gratis, también las empresas**, y pueda ver y modificar el código.
- Que **pague una comisión quien venda Pliego, lo integre en un producto de pago o lo ofrezca como servicio de pago**.
  Ander quiere ser quien cobre por comercializarlo.
- Una licencia **propia, source-available** (no es open source según la OSI), con la **Business Source License 1.1**
  (BUSL-1.1) como base.

Hasta el 2026-11-05 el proyecto era **AGPL-3.0-or-later** y usaba MuPDF (Artifex, AGPL), que impedía cualquier otra
licencia. MuPDF salió del proyecto el 2026-11-05 (#200) y por eso ahora se puede cambiar la licencia.

## 2. Qué incluye el PR

| Archivo | Qué hace |
| --- | --- |
| `LICENSE` | Texto de la BUSL-1.1 con los parámetros rellenados: *Additional Use Grant* (uso gratis salvo «Commercial Offering»), *Change Date* (4 años por versión) y *Change License* (AGPL-3.0-or-later). Licenciante, contacto y fecha de corte quedan `[PENDIENTE]` |
| `LICENSES/AGPL-3.0-or-later.txt` | Texto de la AGPL (el anterior `LICENSE`), que es la licencia de cambio |
| `CLA.md` y `CONTRIBUTING.md` | Acuerdo de contribución y cómo se acepta. `docs/legal/cla-firmantes.md` es el registro |
| `THIRD-PARTY.md` | Avisos de terceros: PDFium y lo que lleva dentro, lopdf, Boa 0.22 con sus transitivas, crates MPL-2.0, IJG, Tesseract, fuentes, modelos, npm |
| `docs/legal/dependencias-rust.md` | Lista de las 514 crates de Rust que se distribuyen (nombre, versión, licencia, ejecutable), generada de `Cargo.lock` |
| `deny.toml` | Quita AGPL de las licencias permitidas; prohíbe `mupdf` y `mupdf-sys`; los crates propios (`publish = false`) llevan BUSL-1.1 |
| `Cargo.toml`, `package.json`, «Acerca de», pie de la app, instalador, README, CLAUDE.md, CONVENTIONS, web | `license = "BUSL-1.1"` y textos que decían «software libre» o AGPL |

Lo que **no** hace: no registra la marca, no crea el contrato comercial, no activa ningún bot de CLA ni publica nada
fuera del repositorio.

## 3. Decisiones que propone el borrador (para que las cuestiones o las confirmes)

1. **BUSL-1.1 con una concesión adicional**. Los *Covenants of Licensor* de la BUSL obligan a no modificar el texto salvo los
   parámetros; la restricción de «vender, integrar, ofrecer como servicio» va, por tanto, como lo que **no** cubre la
   concesión de uso. Definición de *Commercial Offering*: (a) vender, licenciar, alquilar o distribuir copias a cambio de
   algo; (b) incorporarlo a un producto o servicio de pago; (c) alojarlo o prestarlo a terceros como servicio de pago.
   Quedan fuera: usarlo para producir documentos propios o de clientes (una gestoría), cobrar por consultoría, formación,
   instalación o soporte sin cobrar por el programa, redistribuir copias sin cambios y gratis, e investigación y docencia.
2. **Cambio de licencia a los 4 años por versión** (el máximo que permite la BUSL) a **AGPL-3.0-or-later**: mantiene el
   copyleft y es la licencia de lo ya publicado. Alternativa: Apache-2.0 (permisiva, más fácil para terceros, pero deja
   que se cierre el código a los 4 años). Hay que confirmar que la AGPL-3.0 cumple el requisito de compatibilidad con la GPL
   de la BUSL.
3. **CLA como licencia de explotación no exclusiva, sublicenciable e irrevocable**, y no como cesión de la titularidad. El
   Contribuyente conserva su código, y el Licenciante puede relicenciarlo y venderlo (cláusula 3). Se prefiere a la cesión
   exclusiva por la LPI (ver §5) y porque desanima menos a quien colabora; es la pregunta 7.
4. **Sin pagos a contribuyentes** (cláusula 7 del CLA): decisión de Ander, hay que dejarla clara.
5. **Se mantiene el código visible** y cada versión pasa a AGPL (cláusula 6 del CLA): es una promesa a los contribuyentes.

## 4. Estado de la titularidad (hechos)

- `git log`: **un único autor humano**, `Carreraander` (50 commits en este historial, 2026-11-04 y 2026-11-05). No hay
  contribuciones de terceros ni PR externos fusionados. El historial visible empieza el 2026-11-04 (el proyecto se
  trasladó de cuenta ese día); según las notas del proyecto, el repositorio es público desde el 2026-11-02. Conviene
  **confirmar la fecha y el contenido de la primera publicación** y si hay copias fuera de GitHub.
- **Casi todo el código lo ha escrito una IA** (Claude, de Anthropic) por encargo de Ander, que decide el diseño, revisa y
  aprueba: los commits llevan `Co-Authored-By: Claude`. Ander hace de editor y arquitecto. Ver pregunta 2.
- **Quién es el Licenciante**: hoy es Ander como persona física. ¿Conviene una sociedad? (pregunta 8).

## 5. Preguntas para el abogado, por prioridad

1. **Alcance y ejecutabilidad de la BUSL-1.1 con esta concesión adicional.** ¿La definición de *Commercial Offering* es
   suficientemente precisa para España y la UE? Casos dudosos: servicio gratuito financiado con publicidad o datos,
   empaquetarlo en una distribución de Linux/winget/Store sin cobrar, un integrador que lo instala en el equipo del
   cliente y cobra solo por el servicio, uso por administraciones públicas, subcontratación. ¿Es defendible la
   restricción frente a una sociedad extranjera? ¿Hace falta traducir la licencia (la BUSL está en inglés y es un texto
   de MariaDB que no se puede cambiar) o basta una traducción orientativa? ¿Es aplicable la normativa de condiciones
   generales de contratación o de cláusulas abusivas a particulares?
2. **Autoría y titularidad con código escrito por IA.** La LPI (art. 5) considera autor a la persona natural, y la UE exige
   «creación intelectual propia del autor». Si lo escribe una IA con dirección humana, ¿qué parte es protegible? ¿Cómo
   documentar las decisiones humanas (diseño, selección, revisión)? ¿Qué dicen los términos del servicio de la IA sobre la
   titularidad de la salida? Afecta a **cualquier** licencia, también a la AGPL anterior. ¿Hay que ajustar la frase
   «© 2026 el Licenciante» de `LICENSE`?
3. **Corte con la AGPL.** Las versiones y commits ya publicados bajo AGPL-3.0-or-later **seguirán siendo AGPL para quien
   los tenga**, para siempre, aunque Ander cambie la licencia (puede vender o regalar esas copias bajo AGPL). ¿Cómo dejar
   constancia del corte? Propuesta: etiqueta `agpl-final` en el último commit AGPL, `LICENSE` nuevo desde el siguiente, y
   `[PENDIENTE]` de `LICENSE` con ese commit. ¿Se ha distribuido algún binario con MuPDF (la beta
   `Pliego-Setup-0.1.0-beta.exe` y las descargas de pdfpliego.com)? Esas copias llevan la obligación de ofrecer el código de
   esa versión mientras se distribuyan.
4. **Limpieza respecto de MuPDF.** MuPDF y el crate `mupdf` (AGPL, de Artifex y de terceros) se quitaron en #200; ni el
   `Cargo.lock` ni el ejecutable los contienen (comprobado: `cargo deny` los prohíbe y el binario del worker solo trae un
   mensaje de error que nombra «mupdf»). Pero el historial tiene código escrito contra su API, y 160 líneas del código
   actual lo mencionan, casi todas en comentarios. Las reglas de trabajo prohibían copiar o portar su código (se escribió desde
   la especificación PDF). ¿Basta con eso o hace falta un informe de limpieza (*clean-room*) antes de publicar? ¿Se puede
   considerar que el código escrito contra la API del crate `mupdf` es una obra derivada, y ya rehecho no lo es?
5. **Dependencias.** Todas permisivas, y `deny.toml` bloquea las copyleft (ver `THIRD-PARTY.md`). Puntos a mirar: cinco crates
   **MPL-2.0** (`cssparser`, `cssparser-macros`, `dtoa-short`, `selectors`, `option-ext`; solo en la app, vía Tauri y sin
   modificar); la **FTL de FreeType** dentro de `pdfium.dll` (exige mencionarlo); **encoding_rs** (parte BSD-3-Clause con aviso);
   fuentes **Liberation** (OFL-1.1: no se venden aparte, nombres reservados); el **WebView2 Runtime** (no se incluye);
   **Tesseract/Leptonica** y sus DLL de vcpkg; **modelos de IA** que el usuario descarga (Apache-2.0/MIT, no se redistribuyen).
   ¿Falta algún aviso para distribuir el instalador? ¿Se puede usar bajo una licencia source-available algo con MPL-2.0?
6. **Contrato comercial y «comisión».** La BUSL solo dice que hace falta una licencia comercial; precio, regalías o
   porcentaje, reventa, OEM, auditoría y fiscalidad (IVA, retenciones) van en un contrato aparte, que **aún no existe**. ¿Qué
   estructura recomiendas para cobrar a quien lo comercialice (licencia por unidad, porcentaje, cuota)?
7. **CLA.** Revisar `CLA.md`: (a) la LPI (art. 43) exige forma escrita y fija plazo de 5 años y ámbito territorial de España si
   no se dice otra cosa, y declara nulas las cesiones globales de obra futura: el texto dice «todo el mundo», «todo el plazo
   de protección» y limita a contribuciones concretas. (b) ¿licencia no exclusiva sublicenciable, o cesión exclusiva?
   (c) ¿vale la aceptación por comentario de GitHub como firma electrónica? (d) contribuyentes de otros países y
   empleados con derechos del empleador (art. 51 LPI y análogos), (e) tratamiento de datos, (f) ley y fuero aplicables,
   (g) ¿hace falta una versión en inglés con valor contractual?
8. **Titular y marca.** ¿Persona física o sociedad como Licenciante? Registro de la marca «Pliego» (OEPM o EUIPO, clases 9
   y 42; comprobar anterioridades) y del dominio pdfpliego.com. La BUSL no concede derechos sobre marcas (`LICENSE` lo
   dice); ¿conviene una política de uso de la marca?
9. **Comunicación.** No llamar «software libre» ni «open source» a Pliego. Ya se han quitado de la app, el instalador y la
   web; queda por decidir el resto de la web (sigue diciendo «gratuito» y «uso en empresas: permitido»). ¿Alguna cautela por
   publicidad o consumo?
10. **Otros.** Exportación de criptografía (AES-256, firma) desde España/UE, y privacidad del servidor de actualizaciones
    (registro de IP).

## 6. Decisiones de Ander que no son legales

- Nombre legal, NIF y contacto del Licenciante (`LICENSE`, `CLA.md`).
- Licencia de cambio: AGPL (propuesta) o Apache-2.0.
- Si se permite redistribuir copias sin cambios y gratis, y si se acepta publicidad en un servicio gratuito.
- Cómo registrar las aceptaciones del CLA: a mano (propuesta, `docs/legal/cla-firmantes.md`) o con el servicio CLA Assistant
  de GitHub (es un servicio externo, que Ander debe autorizar).
- Qué se hace con el resto de la web (eslóganes «libre y gratuito» y la sección «Licencia»).

## 7. Pasos al aprobar (en este orden)

1. El abogado revisa `LICENSE`, `CLA.md`, este resumen y `THIRD-PARTY.md`; se ajustan los textos.
2. Ander rellena los `[PENDIENTE]` y elige las opciones del apartado 6.
3. Se pone la etiqueta `agpl-final` en el último commit AGPL (permiso de Ander).
4. Se marca el PR como listo y gestión lo fusiona (squash) con CI verde.
5. Se publica la primera versión BUSL-1.1 y se despliega la web.
6. Se registra la marca y se abre el cauce de licencias comerciales.

## 8. Comprobaciones hechas en el PR

- `cargo deny check licenses bans sources`: en verde (`cargo-deny` 0.20.2). `advisories` falla por avisos que ya existían
  y no tienen que ver con la licencia (`rsa`, `zip`, `rustybuzz`, `ttf-parser`...).
- `Cargo.lock` sin `mupdf` ni `mupdf-sys`; `cargo tree -i mupdf` no encuentra el paquete; `strings pliego-worker` solo
  encuentra el mensaje de error que avisa de que `PLIEGO_ENGINE=mupdf` ya no existe.
- No se ha comprobado el instalador de Windows ni los cambios de la app en pantalla (sin equipo Windows en este entorno).

## Fuentes

- Texto de la BUSL-1.1 (SPDX): <https://spdx.org/licenses/BUSL-1.1.html>; plantilla de MariaDB: <https://mariadb.com/bsl11/>
  (no se pudo abrir desde este entorno: **comparar `LICENSE` con la versión vigente**).
- Estudio previo y análisis de alternativas: [`docs/design/licencia-y-motor.md`](../design/licencia-y-motor.md).
- Texto refundido de la Ley de Propiedad Intelectual (RDL 1/1996), arts. 5, 14, 17, 43 y 97-100. Citas a verificar por el
  abogado.
