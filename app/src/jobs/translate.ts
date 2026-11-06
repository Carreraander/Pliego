/**
 * Traducción de los textos de los trabajos (progreso, resumen y avisos del centro de tareas).
 *
 * Los crates del motor y `app/service` escriben esos textos en español (también los usa la CLI). La interfaz los
 * traduce al mostrarlos: cada regla es `[plantilla en español, plantilla en inglés]`; `{}` o `{nombre}` en la plantilla
 * española captura un trozo (número, nombre de archivo, otro texto ya traducible) y en la inglesa se coloca por orden
 * (`{}`) o por posición (`{1}`, `{2}`…). Lo que no tiene regla (nombres de archivo, errores libres) se muestra igual.
 * En español no se toca nada. `translate.test.ts` comprueba las reglas contra ejemplos reales del motor.
 */
import type { Locale } from "../i18n";
import { CONVERT_RULES } from "../i18n/engine/rules-convert";
import { DOCS_RULES } from "../i18n/engine/rules-docs";
import { EDIT_RULES } from "../i18n/engine/rules-edit";
import { PRINT_RULES } from "../i18n/engine/rules-print";
import { SIGN_RULES } from "../i18n/engine/rules-sign";
import { SYSTEM_RULES, type Rule } from "../i18n/engine/rules-system";

/** Progreso (`ctx.step`) y resúmenes (`Report`) de los trabajos. Los errores e informes por áreas están en `i18n/engine/`. */
const JOB_RULES: readonly Rule[] = [
  // ── Progreso (`ctx.step`) ──
  ["Ajustando páginas", "Adjusting pages"], ["Analizando la página", "Analyzing the page"], ["Analizando páginas", "Analyzing pages"],
  ["Aplicando redacciones", "Applying redactions"], ["Añadiendo marcas", "Adding marks"], ["Añadiendo {}", "Adding {}"],
  ["Buscando imágenes", "Looking for images"], ["Buscando líneas finas", "Looking for hairlines"], ["Buscando líneas", "Looking for lines"],
  ["Buscando recuadros", "Looking for boxes"], ["Cifrando…", "Encrypting…"], ["Comparación terminada", "Comparison finished"],
  ["Comparando páginas", "Comparing pages"], ["Comprobación terminada", "Check finished"], ["Comprobando el resultado", "Checking the result"],
  ["Comprobando la descarga", "Verifying the download"], ["Comprobando páginas", "Checking pages"], ["Contenido reescrito", "Content rewritten"],
  ["Convirtiendo a PDF/X con Ghostscript", "Converting to PDF/X with Ghostscript"], ["Descargando el modelo", "Downloading the model"],
  ["Dibujando el informe", "Drawing the report"], ["Escribiendo metadatos", "Writing metadata"], ["Etiquetando", "Tagging"],
  ["Fila {} de {}", "Row {1} of {2}"], ["Firmando", "Signing"], ["Fondo", "Background"], ["Guardando", "Saving"],
  ["Indexando el documento", "Indexing the document"], ["Informe terminado", "Report finished"], ["Leyendo el documento", "Reading the document"],
  ["Leyendo el texto del documento", "Reading the document text"], ["Leyendo {}, página {}", "Reading {1}, page {2}"],
  ["Listo", "Done"], ["Líneas finas corregidas", "Hairlines fixed"], ["Marca de agua", "Watermark"], ["Marcas añadidas", "Marks added"],
  ["Optimizando imágenes", "Optimizing images"], ["Parte {} de {}", "Part {1} of {2}"], ["Poniendo nombre a los campos", "Naming the fields"],
  ["Preparando el documento", "Preparing the document"], ["Preparando la impresión", "Preparing to print"],
  ["Imprimiendo la página {}", "Printing page {}"], ["Enviado a la impresora", "Sent to the printer"],
  ["Preparando el paquete", "Preparing the package"], ["Preparando página {}", "Preparing page {}"], ["Preparando", "Preparing"],
  ["Quitando la protección…", "Removing the protection…"], ["Recomprimiendo imágenes", "Recompressing images"],
  ["Reconociendo página {}", "Recognizing page {}"], ["Redacción aplicada", "Redaction applied"],
  ["Reescribiendo el contenido de las páginas", "Rewriting the page content"], ["Reorganizando páginas", "Rearranging pages"],
  ["Resumiendo el documento", "Summarizing the document"], ["Saneado completado", "Sanitizing finished"], ["Saneando páginas", "Sanitizing pages"],
  ["Terminado", "Finished"], ["analizando la página {}", "analyzing page {}"], ["convirtiendo", "converting"], ["guardando", "saving"],
  ["leyendo capítulos", "reading chapters"], ["leyendo imágenes", "reading images"], ["leyendo la página {}", "reading page {}"],
  ["listo", "done"], ["maquetando", "laying out"], ["página {}", "page {}"], ["sello", "stamp"], ["fin", "end"],
  // ── Resúmenes y avisos (`Report`) ──
  ["Se enviaron {} página(s) a «{}»", "Sent {1} page(s) to “{2}”"],
  ["Se enviaron {} página(s) × {} copias a «{}»", "Sent {1} page(s) × {2} copies to “{3}”"],
  ["«{}» no imprime a doble cara: se imprimió a una cara", "“{}” does not print two-sided: it was printed on one side"],
  ["Cartera con {} archivos.", "Portfolio with {} files."], ["Convertido a {} con la condición «{}».", "Converted to {1} with the condition “{2}”."],
  ["Corregido: {}.", "Fixed: {}."], ["Firmado. Siguiente: {} ({} de {})", "Signed. Next: {1} ({2} of {3})"], ["Firmado. Ya han firmado todos", "Signed. Everyone has signed"], ["Documento cifrado con {}.", "Document encrypted with {}."], ["Documento firmado", "Document signed"],
  ["Enlace actualizado en la página {}", "Link updated on page {}"], ["Enlace añadido en la página {}", "Link added on page {}"],
  ["Enlace borrado de la página {}", "Link removed from page {}"],
  ["Etiquetado: {} encabezado(s), {} párrafo(s), {} lista(s) con {} elemento(s), {} figura(s), {} tabla(s); {} gráfico(s) marcados como decoración.",
    "Tagged: {1} heading(s), {2} paragraph(s), {3} list(s) with {4} item(s), {5} figure(s), {6} table(s); {7} graphic(s) marked as decoration."],
  ["HTML de {} página(s)", "HTML of {} page(s)"], ["Idioma «{}» instalado", "Language “{}” installed"],
  ["Imagen insertada en la página {}", "Image inserted on page {}"], ["Imagen {} borrada de la página {}", "Image {1} deleted from page {2}"],
  ["Imagen {} recortada en la página {}", "Image {1} cropped on page {2}"], ["Imagen {} reemplazada en la página {}", "Image {1} replaced on page {2}"],
  ["Imagen {} transformada en la página {}", "Image {1} transformed on page {2}"],
  ["La página {} no contiene texto reconocible.", "Page {} contains no recognizable text."],
  ["La página {} se reconoció con poca confianza.", "Page {} was recognized with low confidence."],
  ["No se detectó ninguna tabla: se exporta el texto de cada página, una línea por fila.", "No table was detected: the text of each page is exported, one line per row."],
  ["No se encontraron campos", "No fields were found"],
  ["No se encontró nada que redactar; no se ha escrito ningún archivo.", "There was nothing to redact; no file was written."],
  ["Objeto vectorial {} borrado de la página {}", "Vector object {1} deleted from page {2}"],
  ["Objeto vectorial {} transformado en la página {}", "Vector object {1} transformed on page {2}"],
  ["Paquete preparado para {} firmantes", "Package prepared for {} signers"], ["Protección eliminada.", "Protection removed."],
  ["Páginas escaneadas sin texto reconocible ({}): se exportan como imagen. Aplica OCR al PDF antes de exportar para obtener texto editable.",
    "Scanned pages without recognizable text ({}): they are exported as images. Run OCR on the PDF before exporting to get editable text."],
  ["Sigue sin cumplir: {}", "Still non-compliant: {}"], ["Texto añadido en la página {}", "Text added on page {}"],
  ["Texto del bloque {} reemplazado en la página {}", "Text of block {1} replaced on page {2}"],
  ["documento convertido a PDF", "document converted to PDF"],
  ["el texto desbordó la última página y se creó la página {}", "the text overflowed the last page and page {} was created"],
  ["la página {} supera por sí sola el tamaño máximo", "page {} alone exceeds the maximum size"],
  ["las {} primeras páginas no tienen marcador y van en la primera parte", "the first {} pages have no bookmark and go in the first part"],
  ["parte del texto pasó al principio de la página {} (empujando lo que había)", "part of the text moved to the top of page {} (pushing what was there)"],
  ["se reutilizó la fuente original «{}»: el aspecto del texto se conserva", "the original font “{}” was reused: the text keeps its look"],
  ["sello en {} páginas", "stamp on {} pages"], ["texto de {} página(s)", "text of {} page(s)"],
  ["{} comentario(s) importados", "{} comment(s) imported"], ["{} comentario(s) exportados", "{} comment(s) exported"],
  ["{} indexados, {} sin cambios, {} olvidados (extracción {} s, escritura {} s)", "{1} indexed, {2} unchanged, {3} forgotten (extraction {4} s, writing {5} s)"],
  ["{} páginas insertadas", "{} pages inserted"],
  ["{} página(s) sin texto seleccionable (¿escaneadas? el reconocimiento de texto (OCR) puede extraerlo)",
    "{} page(s) without selectable text (scanned? text recognition (OCR) can extract it)"],
  ["{} zona(s) redactada(s) ({} coincidencia(s) de patrones); verificado: el texto ya no se puede extraer.",
    "{1} area(s) redacted ({2} pattern match(es)); verified: the text can no longer be extracted."],
  ["{} página(s) creadas desde {} capítulo(s)", "{1} page(s) created from {2} chapter(s)"], ["{} página(s) creadas", "{} page(s) created"],
  ["{} hoja(s) exportadas a Excel", "{} sheet(s) exported to Excel"],
  ["{} imagen(es) no se pudieron incluir (formato no admitido, como SVG, o dañadas)", "{} image(s) could not be included (unsupported format, such as SVG, or damaged)"],
  ["{} (cómic de {} imágenes)", "{1} (comic of {2} images)"], ["{} PDF generado(s) en {}", "{1} PDF(s) generated in {2}"],
  ["{} archivos combinados, {} páginas", "{1} files merged, {2} pages"], ["{} archivos creados", "{} files created"],
  ["{} archivos extraídos", "{} files extracted"], ["{} campos creados", "{} fields created"],
  ["{} campos de formulario se renombraron (_2, _3…) porque su nombre ya existía en otro archivo",
    "{} form fields were renamed (_2, _3…) because their name already existed in another file"],
  ["{} carácter(es) fuera de Windows-1252 (cirílico, griego, CJK, emojis…) se sustituyeron por «?»",
    "{} character(s) outside Windows-1252 (Cyrillic, Greek, CJK, emoji…) were replaced with “?”"],
  ["{} diapositiva(s) exportadas a PowerPoint", "{} slide(s) exported to PowerPoint"],
  ["{} figura(s) necesitan texto alternativo: usa `pliego a11y check` para ver cuáles.", "{} figure(s) need alternative text: use `pliego a11y check` to see which."],
  ["{} imagen(es) exportadas", "{} image(s) exported"], ["{} imagen(es) extraídas", "{} image(s) extracted"],
  ["{} procesados, {} reanudados, {} con error", "{1} processed, {2} resumed, {3} failed"], ["{} procesados, {} con error", "{1} processed, {2} failed"],
  ["{} página(s) exportadas a Word; {} tabla(s) detectada(s)", "{1} page(s) exported to Word; {2} table(s) detected"],
  ["{} página(s) reconocida(s), {} omitida(s) por tener ya texto, {} palabras.", "{1} page(s) recognized, {2} skipped because they already have text, {3} words."],
  ["{} páginas numeradas", "{} pages numbered"], ["{} páginas", "{} pages"], ["{} (en {})", "{1} (in {2})"],
  ["{}: se reintenta tras archivos ya procesados; si el lote usa numeración continua puede repetir números (relanza sin --resume para renumerar todo)",
    "{}: retrying after already processed files; if the batch uses continuous numbering, numbers may repeat (rerun without --resume to renumber everything)"],
  ["«{}» no se pudo leer", "“{}” could not be read"],
];

/** Prefijo genérico («archivo: mensaje»): va el último para no tapar a los demás; traduce lo que sigue. */
const GENERIC: Rule = ["{}: {}", "{1}: {2}"];

/** Todas las reglas. Se aplican de la más específica (más texto fijo) a la más general; la genérica, la última. */
export const RULES: readonly Rule[] = [...JOB_RULES, ...SYSTEM_RULES, ...SIGN_RULES, ...DOCS_RULES, ...EDIT_RULES, ...CONVERT_RULES, ...PRINT_RULES, GENERIC];

interface Compiled { re: RegExp; en: string; fixed: number }

/** `Fila {} de {}` → `/^Fila (.+?) de (.+?)$/is`. */
function compile([es, en]: Rule): Compiled {
  const escaped = es.replace(/[.*+?^$|()[\]\\]/g, "\\$&").replace(/\{\w*\}/g, "(.+?)");
  return { re: new RegExp(`^${escaped}$`, "s"), en, fixed: es.replace(/\{\w*\}/g, "").length };
}

let compiled: Compiled[] | null = null;

/** Rellena `{}` (por orden) y `{n}` (por posición, desde 1) de una plantilla inglesa. */
function fill(template: string, args: string[]): string {
  let next = 0;
  return template.replace(/\{(\d*)\}/g, (_, n: string) => args[n ? Number(n) - 1 : next++] ?? "");
}

/** Reglas compiladas: la genérica va siempre la última; el resto, de más a menos texto fijo (el orden de escritura desempata). */
function rules(): Compiled[] {
  if (!compiled) {
    const generic = compile(GENERIC);
    const rest = RULES.filter((r) => r !== GENERIC).map(compile);
    compiled = [...rest.map((c, i) => ({ c, i })).sort((a, b) => b.c.fixed - a.c.fixed || a.i - b.i).map((x) => x.c), generic];
  }
  return compiled;
}

/** Traduce un trozo capturado conservando los espacios que lo rodean (` (página 3)` sigue pegado a lo anterior). */
function part(arg: string, locale: Locale): string {
  const lead = /^\s*/.exec(arg)![0];
  const tail = /\s*$/.exec(arg)![0];
  return `${lead}${jobText(arg, locale)}${arg.trim() ? tail : ""}`;
}

/** Texto de un trabajo, error o informe del motor en el idioma de la app. En español, o sin regla que lo cubra, queda tal cual. */
export function jobText(text: string, locale: Locale): string {
  if (locale === "es" || !text) return text;
  const trimmed = text.replace(/\s+/g, " ").trim();
  for (const { re, en } of rules()) {
    const m = re.exec(trimmed);
    if (m) return fill(en, m.slice(1).map((a) => part(a, locale)));
  }
  return text;
}
