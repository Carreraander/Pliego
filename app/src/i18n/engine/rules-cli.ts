import type { Rule } from "./rules-system";

/** Mensajes de ejecución de la CLI (`pliego …`): lo que imprime o devuelve como error. La CLI usa estas mismas reglas desde Rust. */
export const CLI_RULES: readonly Rule[] = [
  // ── Datos del documento (`pliego info`) ──
  ["Páginas:", "Pages:"], ["Título:", "Title:"], ["Autor:", "Author:"], ["Asunto:", "Subject:"], ["Creador:", "Creator:"],
  ["Productor:", "Producer:"], ["Formato:", "Format:"], ["Cifrado:", "Encryption:"],
  ["Página {}: {} coincidencia(s)", "Page {1}: {2} match(es)"],
  // ── Accesibilidad ──
  ["«{}»: usa el formato OBJETO=texto", "“{}”: use the format OBJECT=text"], ["«{}» no es un número de objeto", "“{}” is not an object number"],
  ["{}: {} página(s), {}.", "{1}: {2} page(s), {3}."], 
  ["el documento no cumple las comprobaciones de accesibilidad", "the document does not pass the accessibility checks"],
  // ── Anotaciones ──
  ["las páginas empiezan en 1", "pages start at 1"], ["indica -o <salida.pdf> (o --in-place para modificar el original)", "give -o <output.pdf> (or --in-place to modify the original)"],
  ["el JSON de la anotación no es válido", "the annotation JSON is not valid"], ["Anotación {} añadida en la página {}: {}", "Annotation {1} added on page {2}: {3}"],
  ["Anotación {} borrada: {}", "Annotation {1} deleted: {2}"], ["{} anotación(es) aplanada(s): {}", "{1} annotation(s) flattened: {2}"],
  // ── Comparar ──
  ["Páginas: {} → {} · insertadas {} · eliminadas {} · con cambios {}", "Pages: {1} → {2} · inserted {3} · removed {4} · changed {5}"],
  ["[{} → {}] página insertada", "[{1} → {2}] page inserted"], ["[{} → {}] página eliminada", "[{1} → {2}] page removed"],
  ["formato «{}» ({})", "format “{1}” ({2})"], ["añadido «{}»", "added “{}”"], ["borrado «{}»", "deleted “{}”"], ["cambiado «{}» → «{}»", "changed “{1}” → “{2}”"],
  // ── Convertir y editar ──
  ["indica al menos un archivo de entrada", "give at least one input file"], ["{} página(s) en JSON", "{} page(s) in JSON"],
  ["el recuadro debe ser x0,y0,x1,y1 con números", "the box must be x0,y0,x1,y1 with numbers"], ["el recuadro debe tener cuatro números: x0,y0,x1,y1", "the box must have four numbers: x0,y0,x1,y1"],
  ["el color debe ser #RRGGBB", "the color must be #RRGGBB"], ["color no válido", "invalid color"],
  ["Sin bloques de texto editables.", "No editable text blocks."], ["Sin imágenes.", "No images."],
  // ── Formularios ──
  ["El documento no tiene campos de formulario.", "The document has no form fields."], ["el JSON de datos no es válido", "the data JSON is not valid"],
  ["«{}» debe ser CAMPO=VALOR", "“{}” must be FIELD=VALUE"], ["indica valores con --set CAMPO=VALOR o --data datos.json", "give values with --set FIELD=VALUE or --data data.json"],
  ["no se reconoce el formato de datos; indica --format json|csv|fdf|xfdf", "the data format is not recognized; give --format json|csv|fdf|xfdf"],
  ["usa cuatro números separados por comas: x0,y0,x1,y1", "use four numbers separated by commas: x0,y0,x1,y1"],
  ["{} {} pág. {} [{} {} {} {}]{}", "{1} {2} p. {3} [{4} {5} {6} {7}]{8}"], ["pág. {} {} {} [{} {} {} {}]", "p. {1} {2} {3} [{4} {5} {6} {7}]"],
  ["Campo añadido", "Field added"], ["Cálculo guardado", "Calculation saved"], ["Validación guardada", "Validation saved"], ["la máscara", "the mask"],
  ["{} campo(s) detectado(s). Usa -o <salida.pdf> para crearlos.", "{} field(s) detected. Use -o <output.pdf> to create them."], ["Orden de tabulación fijado", "Tab order set"],
  // ── Medir ──
  ["«{}» necesita una unidad (mm, cm, m, km, in, ft, pt)", "“{}” needs a unit (mm, cm, m, km, in, ft, pt)"], ["número no válido en «{}»", "invalid number in “{}”"],
  ["punto «{}»: usa x,y", "point “{}”: use x,y"], ["indica la medida del papel con --paper o --points", "give the measure on paper with --paper or --points"],
  ["la página {} no tiene escala: calíbrala antes con `pliego measure scale`", "page {} has no scale: calibrate it first with `pliego measure scale`"],
  ["Página {}: escala {}", "Page {1}: scale {2}"], ["Página {}: {}", "Page {1}: {2}"],
  // ── OCR, optimizar ──
  ["falta el PDF de entrada", "the input PDF is missing"], ["Imágenes", "Images"], ["Tamaño", "Size"], ["Después", "After"],
  ["Imágenes: {} · objetos: {}", "Images: {1} · objects: {2}"], ["falta el archivo de salida: usa -o salida.pdf (o --analyze para solo analizar)", "the output file is missing: use -o output.pdf (or --analyze to only analyze)"],
  ["Imágenes: {} · recomprimidas {} (reducidas {}) · sin tocar {}", "Images: {1} · recompressed {2} (downsampled {3}) · untouched {4}"],
  // ── Páginas ──
  ["tamaño no válido: {}", "invalid size: {}"], ["el tamaño debe ser mayor que 0", "the size must be greater than 0"], ["tramo no válido: {}", "invalid range: {}"],
  ["las páginas se numeran desde 1", "pages are numbered from 1"], ["número no válido: {}", "invalid number: {}"], ["regla no válida: {}", "invalid rule: {}"],
  ["falta el estilo en la regla {}", "the style is missing in rule {}"], ["número inicial no válido: {}", "invalid starting number: {}"],
  ["{} páginas extraídas", "{} pages extracted"], ["posición no válida: {} (usa un número de 1 a {} o \"end\")", "invalid position: {1} (use a number from 1 to {2} or \"end\")"],
  ["{} páginas en blanco insertadas", "{} blank pages inserted"], ["{} páginas borradas", "{} pages deleted"], ["{} páginas giradas {}°", "{1} pages rotated {2}°"],
  ["página {} movida a la posición {}", "page {1} moved to position {2}"], ["páginas reordenadas", "pages reordered"], ["{} páginas duplicadas", "{} pages duplicated"],
  ["{} páginas recortadas", "{} pages cropped"],
  // ── PDF/A, cartera, impresión ──
  ["- {} cláusula {} ({} fallo(s)): {}", "- {1} clause {2} ({3} failure(s)): {4}"], ["{}: cumple {} según las reglas comprobadas.", "{1}: complies with {2} according to the rules checked."],
  ["el documento no cumple {}", "the document does not comply with {}"], ["veraPDF dice que el resultado no cumple {}", "veraPDF says the result does not comply with {}"],
  ["Sin archivos incrustados.", "No embedded files."], ["PDF con archivos adjuntos", "PDF with attached files"], ["Impresión comercial", "Commercial printing"],
  ["El documento no usa tintas de proceso ni colores directos (solo RGB o grises).", "The document uses no process inks or spot colors (only RGB or grays)."],
  ["{} {} páginas {}", "{1} {2} pages {3}"], ["perfil desconocido «{}» (usa basic, print o pdfx)", "unknown profile “{}” (use basic, print or pdfx)"],
  ["Perfil «{}»: {} error(es), {} aviso(s) en {} página(s).", "Profile “{1}”: {2} error(s), {3} warning(s) in {4} page(s)."],
  ["el documento no cumple el perfil «{}»", "the document does not meet the “{}” profile"], ["Cobertura total máxima de tinta: {} %", "Maximum total ink coverage: {} %"],
  ["{} línea(s) corregidas en {} página(s); mínimo {} pt.", "{1} line(s) fixed in {2} page(s); minimum {3} pt."],
  ["Marcas añadidas en {} página(s); sangrado de {} mm.", "Marks added in {1} page(s); bleed of {2} mm."],
  // ── Proteger, redactar, acciones ──
  ["indica --owner-password: sin ella cualquiera podría quitar las restricciones", "give --owner-password: without it anyone could remove the restrictions"], ["copiar texto e imágenes", "copy text and images"],
  ["formato esperado PÁGINA:X0,Y0,X1,Y1", "expected format PAGE:X0,Y0,X1,Y1"], ["la página debe ser un número", "the page must be a number"], ["«{}» no es un número", "“{}” is not a number"],
  ["hacen falta 4 números: X0,Y0,X1,Y1", "4 numbers are needed: X0,Y0,X1,Y1"], ["indica qué redactar con --pattern y/o --area", "say what to redact with --pattern and/or --area"],
  ["con --apply indica el archivo de salida con -o", "with --apply, give the output file with -o"], ["falta la acción", "the action is missing"],
  ["falta la carpeta de salida: usa -o carpeta (los originales nunca se modifican)", "the output folder is missing: use -o folder (the originals are never modified)"],
  // ── Firmas ──
  ["{} no es un número", "{} is not a number"], ["--store solo está disponible en Windows", "--store is only available on Windows"],
  ["B-LT (validación a largo plazo)", "B-LT (long-term validation)"], ["B-LTA (sello de tiempo del documento)", "B-LTA (document timestamp)"],
  ["raíz sin confianza", "untrusted root"], ["cadena no válida", "invalid chain"], ["El documento no contiene firmas.", "The document contains no signatures."],
  ["VÁLIDA", "VALID"], ["ÍNTEGRA con avisos", "INTACT with warnings"], ["certificación DocMDP: nivel {}", "DocMDP certification: level {}"], ["válido", "valid"], ["no válido", "invalid"],
  // ── Avisos y etiquetas sueltas ──
  ["aviso", "warning"], ["Aviso", "Warning"], ["error", "error"], ["campo: {}", "field: {}"], ["fecha: {}", "date: {}"], ["certificado: {}", "certificate: {}"], ["nivel: {}", "level: {}"],
  ["sello de tiempo: {} ({})", "timestamp: {1} ({2})"], ["aviso: {}", "warning: {}"], ["permitido", "allowed"], ["denegado", "denied"], ["sin fecha", "no date"],
  ["instalado", "installed"], ["(ROTA)", "(BROKEN)"], ["(fuera de orden)", "(out of order)"], ["sin etiquetar", "untagged"], ["etiquetado", "tagged"],
  ["(página {}, objeto {})", "(page {1}, object {2})"], ["(página {})", "(page {})"], ["(objeto {})", "(object {})"],
  ["{} error(es), {} aviso(s).", "{1} error(s), {2} warning(s)."], ["Cómo arreglarlo:", "How to fix it:"],
  // ── Más mensajes de la CLI ──
  ["Sin anotaciones.", "No annotations."], ["{} comentario(s) importado(s): {}", "{1} comment(s) imported: {2}"], ["Carpeta de modelos: {}", "Models folder: {}"],
  ["Sin diferencias.", "No differences."], ["Palabras: +{} −{} ~{} · formato {} · zonas visuales {}", "Words: +{1} −{2} ~{3} · format {4} · visual areas {5}"],
  ["[{} → {}] {} zona(s) con diferencias visuales", "[{1} → {2}] {3} area(s) with visual differences"], ["Informe: {}", "Report: {}"],
  ["Sin objetos vectoriales.", "No vector objects."], ["indica --uri o --to-page", "give --uri or --to-page"],
  ["{} campo(s) rellenado(s): {}", "{1} field(s) filled in: {2}"], ["{} campo(s) restablecido(s): {}", "{1} field(s) reset: {2}"], ["{} campo(s) aplanado(s): {}", "{1} field(s) flattened: {2}"],
  ["{} necesita --pattern", "{} needs --pattern"], ["{} campo(s) recalculado(s): {}", "{1} field(s) recalculated: {2}"], ["{} campo(s) creado(s): {}", "{1} field(s) created: {2}"],
  ["Sin resultados.", "No results."], ["Sin escalas ni medidas.", "No scales or measurements."], ["Modelo instalado: {}", "Model installed: {}"],
  ["{} Guardado en {}", "{1} Saved to {2}"], ["indica -o <salida> o --in-place", "give -o <output> or --in-place"], ["estilo de etiqueta desconocido: {}", "unknown label style: {}"],
  ["indica --from <pdf> o --blank <n>", "give --from <pdf> or --blank <n>"], ["indica --rect o --margins", "give --rect or --margins"],
  ["{} reglas de etiquetas aplicadas", "{} label rules applied"], ["veraPDF: cumple.", "veraPDF: compliant."], ["veraPDF: no cumple.", "veraPDF: not compliant."],
  ["{}: no cumple {}:", "{1}: does not comply with {2}:"], ["indique --p12 o --store", "give --p12 or --store"], ["Firma {}: {} · {}", "Signature {1}: {2} · {3}"],
  ["Le toca firmar a {} ({} de {})", "It is {1}'s turn to sign ({2} of {3})"], ["{} Preparado para: {}", "{1} Prepared for: {2}"],
  ["{} Firma de {}{}{} huella {}", "{1} Signed by {2}{3}{4} fingerprint {5}"], ["indica --text o --image", "give --text or --image"], ["indica --color o --image", "give --color or --image"],
  ["esquina desconocida: {}", "unknown corner: {}"], ["Sin coincidencias.", "No matches."],
  ["Ya han firmado todos.", "Everyone has signed."], ["Firmado. Siguiente: {} ({} de {})", "Signed. Next: {1} ({2} of {3})"], ["Firmado. Ya han firmado todos.", "Signed. Everyone has signed."],
  ["Fuentes", "Fonts"], ["Contenido de páginas", "Page content"], ["Otros flujos", "Other streams"], ["Estructura", "Structure"], ["sin filtro", "no filter"],
  ["- {}", "- {}"], ["· {}", "· {}"], ["• {}", "• {}"],
  ["Aviso: el campo «{}» no existe.", "Warning: the field “{}” does not exist."],
  ["Aviso: el campo «{}» no admitió el valor (solo lectura, opción o validación).", "Warning: the field “{}” did not accept the value (read-only, option or validation)."],
  ["{} coincidencia(s). Añade --apply -o <salida.pdf> para redactar.", "{} match(es). Add --apply -o <output.pdf> to redact."],
  ["[{} → {}] {}", "[{1} → {2}] {3}"], ["ERROR {}", "ERROR {}"], ["AVISO {}", "WARNING {}"], ["INFO {}", "INFO {}"],
  // ── Sellos ──
  ["color no válido: {} (usa #RRGGBB)", "invalid color: {} (use #RRGGBB)"], ["color no válido: {}", "invalid color: {}"], ["color no válido: {} (usa r,g,b)", "invalid color: {} (use r,g,b)"],
  ["los componentes del color van de 0 a 1", "color components range from 0 to 1"],
];
