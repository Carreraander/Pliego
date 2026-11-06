/**
 * Herramientas de base: las que ya existen en `main` (W01) y el mapa completo de §4.3 como herramientas «fantasma»,
 * para que la paleta muestre ya todo lo que vendrá. Cada flujo sustituye la suya registrando un manifiesto con el mismo
 * `id` (sin `ghost`) desde su carpeta `app/src/tools/<area>/`.
 */
import { addToolStrings } from "./strings";
import { getTool, registerTool, type ToolGroup, type ToolKind, type ToolManifest } from "./registry";

/** [id, grupo, tipo, icono, [es título, es descripción], [en título, en descripción], palabras clave, requisitos, atajo]. */
type Row = [string, ToolGroup, ToolKind, string, [string, string], [string, string], string[], ToolManifest["when"]?, string?];

const D = { document: true } as const;

const rows: Row[] = [
  // Ver (W01)
  ["viewer.present", "view", "mode", "presentation", ["Presentación", "Muestra el documento a pantalla completa"], ["Presentation", "Show the document full screen"], ["pantalla completa", "slides", "fullscreen"], D, "F11"],
  ["viewer.layers", "view", "panel", "layers", ["Capas", "Muestra u oculta las capas del documento"], ["Layers", "Show or hide the document layers"], ["layers", "ocg"], D],
  ["viewer.attachments", "view", "panel", "paperclip", ["Adjuntos", "Archivos incrustados en el PDF"], ["Attachments", "Files embedded in the PDF"], ["adjuntar", "attachments", "incrustados"], D],
  ["viewer.properties", "view", "task", "info", ["Propiedades del documento", "Título, autor, tamaño y metadatos"], ["Document properties", "Title, author, size and metadata"], ["metadatos", "autor", "info"], D, "Ctrl+D"],
  // Comentar (W03, W04)
  ["annotate.comment", "comment", "mode", "message-square", ["Comentar", "Resalta, subraya y anota el documento"], ["Comment", "Highlight, underline and annotate"], ["anotar", "resaltar", "subrayar", "nota"], D],
  ["stamp.stamps", "comment", "mode", "stamp", ["Sellos", "Coloca sellos sobre las páginas"], ["Stamps", "Place stamps on pages"], ["sello", "stamp"], D],
  ["annotate.xfdf", "comment", "task", "arrow-left-right", ["Importar o exportar comentarios", "Intercambia comentarios en formato XFDF"], ["Import or export comments", "Exchange comments as XFDF"], ["xfdf", "comentarios"], D],
  ["annotate.summary", "comment", "task", "list", ["Resumen de comentarios", "Lista todos los comentarios del documento"], ["Comments summary", "List every comment in the document"], ["resumen", "comentarios"], D],
  // Rellenar y firmar (W03, W05, W17)
  ["forms.fill", "fill-sign", "mode", "pen-line", ["Rellenar y firmar", "Rellena campos y firma a mano"], ["Fill and sign", "Fill fields and sign by hand"], ["rellenar", "formulario", "firma"], D],
  ["sign.digital", "fill-sign", "wizard", "sign-digital", ["Firma digital", "Firma con certificado o DNIe"], ["Digital signature", "Sign with a certificate or national ID"], ["certificado", "dnie", "pades", "firmar"], D],
  ["sign.validate", "fill-sign", "panel", "badge-check", ["Validar firmas", "Comprueba las firmas del documento"], ["Validate signatures", "Check the document signatures"], ["firmas", "validar"], { document: true, signatures: true }],
  ["sign.workflow", "fill-sign", "wizard", "users", ["Flujo de firmas", "Pide firmas a varias personas sin nube"], ["Signature workflow", "Request signatures from several people"], ["pedir firmas", "workflow"], D],
  // Organizar (W04)
  ["pages.organize", "organize", "mode", "layout-grid", ["Organizar", "Reordena, gira y elimina páginas"], ["Organize", "Reorder, rotate and delete pages"], ["páginas", "rotar", "eliminar", "reordenar"], D],
  ["pages.merge", "organize", "task", "combine", ["Combinar archivos", "Une varios PDF en uno"], ["Merge files", "Join several PDFs into one"], ["unir", "merge", "juntar"]],
  ["pages.split", "organize", "task", "scissors", ["Dividir", "Separa el documento en varios archivos"], ["Split", "Split the document into several files"], ["separar", "split", "partir"], D],
  ["pages.insert", "organize", "task", "file-plus", ["Insertar páginas", "Añade páginas de otro archivo o en blanco"], ["Insert pages", "Add pages from a file or blank"], ["insertar", "añadir"], D],
  ["pages.number", "organize", "task", "hash", ["Numerar páginas", "Pone número de página y encabezados"], ["Number pages", "Add page numbers"], ["numeración", "paginar"], D],
  ["pages.crop", "organize", "task", "crop", ["Recortar páginas", "Quita márgenes de las páginas"], ["Crop pages", "Trim the page margins"], ["recortar", "crop", "márgenes"], D],
  // Editar (W11, W04)
  ["edit.content", "edit", "mode", "type", ["Editar", "Cambia texto e imágenes del documento"], ["Edit", "Change the document text and images"], ["texto", "imagen", "editar"], D, "E"],
  ["edit.watermark", "edit", "task", "droplet", ["Marca de agua", "Añade una marca de agua a las páginas"], ["Watermark", "Add a watermark to pages"], ["watermark"], D],
  ["edit.headerfooter", "edit", "task", "panel-top", ["Encabezado y pie", "Texto repetido arriba y abajo"], ["Header and footer", "Text repeated at the top and bottom"], ["header", "footer"], D],
  ["edit.background", "edit", "task", "image", ["Fondo", "Color o imagen de fondo de las páginas"], ["Background", "Page colour or image"], ["background"], D],
  ["edit.bates", "edit", "task", "bates", ["Numeración Bates", "Numera documentos para uso legal"], ["Bates numbering", "Number documents for legal use"], ["bates", "legal"], D],
  // Formularios (W13)
  ["forms.design", "forms", "mode", "form-input", ["Diseñar formulario", "Crea y edita campos de formulario"], ["Design form", "Create and edit form fields"], ["campos", "formulario", "diseñador"], D],
  ["forms.detect", "forms", "task", "scan-search", ["Detectar campos", "Encuentra campos en un documento plano"], ["Detect fields", "Find fields in a flat document"], ["detectar", "campos"], D],
  // Escanear y OCR (W06)
  ["ocr.recognize", "scan-ocr", "task", "ocr", ["Reconocer texto (OCR)", "Convierte escaneos en texto seleccionable"], ["Recognize text (OCR)", "Turn scans into selectable text"], ["ocr", "escaneo", "reconocer"], D],
  ["ocr.clean", "scan-ocr", "task", "wand", ["Enderezar y limpiar", "Mejora páginas escaneadas torcidas o sucias"], ["Deskew and clean", "Improve skewed or dirty scans"], ["enderezar", "limpiar", "deskew"], D],
  ["ocr.languages", "scan-ocr", "task", "languages", ["Idiomas de OCR", "Elige los idiomas del reconocimiento"], ["OCR languages", "Choose the recognition languages"], ["idiomas", "ocr"]],
  // Proteger (W07)
  ["protect.protect", "protect", "mode", "lock", ["Proteger", "Contraseña, permisos y cifrado"], ["Protect", "Password, permissions and encryption"], ["contraseña", "cifrar", "permisos"], D],
  ["protect.redact", "protect", "mode", "redact", ["Redactar", "Elimina de forma permanente texto e imágenes"], ["Redact", "Permanently remove text and images"], ["redactar", "tapar", "datos personales"], D],
  ["protect.sanitize", "protect", "task", "eraser", ["Sanear documento", "Quita metadatos y contenido oculto"], ["Sanitize document", "Remove metadata and hidden content"], ["sanear", "limpiar", "metadatos"], D],
  // Convertir (W09, W14, W12)
  ["convert.create", "convert", "task", "file-plus-2", ["Crear PDF", "Crea un PDF desde imágenes u otros archivos"], ["Create PDF", "Create a PDF from images or other files"], ["crear", "imágenes", "convertir"]],
  ["convert.export", "convert", "task", "file-output", ["Exportar a imágenes o texto", "Guarda las páginas como imágenes o texto"], ["Export to images or text", "Save pages as images or text"], ["exportar", "png", "txt"], D],
  ["convert.office", "convert", "task", "file-type", ["Exportar a Word, Excel o PowerPoint", "Convierte a formatos de Office (beta)"], ["Export to Word, Excel or PowerPoint", "Convert to Office formats (beta)"], ["word", "excel", "powerpoint", "office", "docx"], D],
  ["convert.pdfa", "convert", "task", "pdf-a", ["Guardar como PDF/A", "Archivo de larga conservación"], ["Save as PDF/A", "Long-term archive format"], ["pdf/a", "archivo", "conservación"], D],
  // Comparar y optimizar (W08)
  ["compare.compare", "compare-optimize", "screen", "columns-2", ["Comparar", "Compara dos documentos y lista los cambios"], ["Compare", "Compare two documents and list changes"], ["diferencias", "comparar"]],
  ["optimize.compress", "compare-optimize", "task", "file-down", ["Comprimir", "Reduce el tamaño del archivo"], ["Compress", "Make the file smaller"], ["reducir", "tamaño", "optimizar", "compress"], D],
  ["optimize.advanced", "compare-optimize", "task", "sliders-horizontal", ["Optimizar avanzado", "Controla imágenes, fuentes y estructura"], ["Advanced optimize", "Control images, fonts and structure"], ["optimizar", "subconjunto", "fuentes"], D],
  // Estándares y accesibilidad (W12)
  ["access.check", "standards", "mode", "accessibility", ["Comprobar accesibilidad", "Revisa etiquetas y estructura"], ["Check accessibility", "Review tags and structure"], ["accesibilidad", "pdf/ua", "etiquetas"], D],
  ["access.fix", "standards", "task", "wand", ["Hacer accesible", "Etiqueta y pone título, idioma y textos alternativos"], ["Make accessible", "Tag and set title, language and alt text"], ["accesibilidad", "etiquetar", "texto alternativo"], D],
  ["access.order", "standards", "mode", "list-ordered", ["Orden de lectura", "Define el orden en que se lee el documento"], ["Reading order", "Set the order the document is read in"], ["lectura", "orden"], D],
  ["access.validate-pdfa", "standards", "task", "shield-check", ["Validar PDF/A", "Comprueba la conformidad con PDF/A"], ["Validate PDF/A", "Check PDF/A conformance"], ["pdf/a", "validar"], { document: true, pdfa: true }],
  // Impresión (W16)
  ["print.preview", "print", "mode", "printer", ["Vista previa de salida", "Separaciones, sobreimpresión y tinta total"], ["Output preview", "Separations, overprint and total ink"], ["imprenta", "separaciones", "cmyk"], D],
  ["print.preflight", "print", "task", "clipboard-check", ["Comprobación previa", "Revisa el documento antes de imprimir"], ["Preflight", "Check the document before printing"], ["preflight", "imprenta"], D],
  // Automatizar (W15)
  ["batch.actions", "automate", "wizard", "list-checks", ["Acciones en lote", "Aplica una serie de pasos a muchos archivos"], ["Batch actions", "Apply a series of steps to many files"], ["lote", "batch", "automatizar"]],
  ["batch.index", "automate", "wizard", "database", ["Índice de búsqueda", "Crea un índice para buscar en muchos PDF"], ["Search index", "Build an index to search many PDFs"], ["índice", "indexar"]],
  ["batch.measure", "automate", "mode", "ruler", ["Medir", "Mide distancias y áreas sobre planos"], ["Measure", "Measure distances and areas on plans"], ["medidas", "planos", "escala"], D],
  ["batch.portfolio", "automate", "mode", "folders", ["Carteras", "Agrupa varios documentos en una cartera"], ["Portfolios", "Group several documents in a portfolio"], ["cartera", "portfolio"]],
  // Asistente (W18)
  ["assistant.panel", "assistant", "panel", "sparkles", ["Asistente local", "Pregunta sobre el documento sin salir del equipo"], ["Local assistant", "Ask about the document without leaving your PC"], ["ia", "resumen", "preguntar"], D],
];

export function registerBaseTools(): void {
  // Leer: ya lo registra la app como modo `read`; aquí solo su manifiesto.
  addToolStrings({
    es: { "tools.read.title": "Leer", "tools.read.description": "Ver, desplazarse y buscar en el documento" },
    en: { "tools.read.title": "Read", "tools.read.description": "View, scroll and search the document" },
  });
  registerTool({ id: "viewer.read", group: "view", kind: "mode", title: "tools.read.title", description: "tools.read.description", icon: "mouse-pointer", keywords: ["ver", "leer", "visor", "read"], modeId: "read", pinnable: true });

  const es: Record<string, string> = {};
  const en: Record<string, string> = {};
  for (const [id, group, kind, icon, [esT, esD], [enT, enD], keywords, when, shortcut] of rows) {
    es[`tools.${id}.title`] = esT; es[`tools.${id}.description`] = esD;
    en[`tools.${id}.title`] = enT; en[`tools.${id}.description`] = enD;
    if (getTool(id) && !getTool(id)!.ghost) continue; // ya la registró su flujo
    registerTool({ id, group, kind, icon, title: `tools.${id}.title`, description: `tools.${id}.description`, keywords, when, shortcut, pinnable: true, ghost: true });
  }
  addToolStrings({ es, en });
}
