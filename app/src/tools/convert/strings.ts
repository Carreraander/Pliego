import { scoped } from "../../settings/text";

export const convertStrings = {
  es: {
    "convert.noDoc": "Abre un documento para convertirlo.",
    "convert.pages": "Páginas", "convert.pages.hint": "Todas, o por ejemplo 1-3,5 o last", "convert.pages.bad": "Rango de páginas no válido.",
    "convert.export.title": "Exportar a imágenes o texto", "convert.export.format": "Formato",
    "convert.export.png": "Imágenes PNG", "convert.export.jpg": "Imágenes JPEG", "convert.export.tiff": "Imágenes TIFF",
    "convert.export.svg": "Vectores SVG", "convert.export.images": "Imágenes incrustadas (originales)",
    "convert.export.txt": "Texto (.txt)", "convert.export.html": "Página web (.html)",
    "convert.export.dpi": "Resolución (ppp)", "convert.export.dpi.hint": "Vacío = 150 (36 a 1200)",
    "convert.export.quality": "Calidad JPEG", "convert.export.quality.hint": "Vacío = 90 (1 a 100)",
    "convert.export.run": "Exportar", "convert.export.label.images": "imágenes", "convert.export.label.text": "texto",
    "convert.export.job": "Exportar {name}", "convert.export.started": "Exportando… sigue en el centro de tareas",
    "convert.export.note": "Se crea una copia junto al original; el PDF no cambia.",
    "convert.office.title": "Exportar a Word, Excel o PowerPoint", "convert.office.format": "Formato",
    "convert.office.docx": "Word (.docx)", "convert.office.xlsx": "Excel (.xlsx)", "convert.office.pptx": "PowerPoint (.pptx)",
    "convert.office.scanned": "Si hay páginas escaneadas, avisarme en vez de exportarlas como imagen",
    "convert.office.beta": "Función beta: el formato puede variar respecto al original.",
    "convert.office.run": "Exportar", "convert.office.label": "Office",
    "convert.office.job": "Exportar {name} a Office", "convert.office.started": "Exportando a Office… sigue en el centro de tareas",
  },
  en: {
    "convert.noDoc": "Open a document to convert it.",
    "convert.pages": "Pages", "convert.pages.hint": "All, or for example 1-3,5 or last", "convert.pages.bad": "Invalid page range.",
    "convert.export.title": "Export to images or text", "convert.export.format": "Format",
    "convert.export.png": "PNG images", "convert.export.jpg": "JPEG images", "convert.export.tiff": "TIFF images",
    "convert.export.svg": "SVG vectors", "convert.export.images": "Embedded images (originals)",
    "convert.export.txt": "Text (.txt)", "convert.export.html": "Web page (.html)",
    "convert.export.dpi": "Resolution (dpi)", "convert.export.dpi.hint": "Empty = 150 (36 to 1200)",
    "convert.export.quality": "JPEG quality", "convert.export.quality.hint": "Empty = 90 (1 to 100)",
    "convert.export.run": "Export", "convert.export.label.images": "images", "convert.export.label.text": "text",
    "convert.export.job": "Export {name}", "convert.export.started": "Exporting… follow it in the task center",
    "convert.export.note": "A copy is created next to the original; the PDF does not change.",
    "convert.office.title": "Export to Word, Excel or PowerPoint", "convert.office.format": "Format",
    "convert.office.docx": "Word (.docx)", "convert.office.xlsx": "Excel (.xlsx)", "convert.office.pptx": "PowerPoint (.pptx)",
    "convert.office.scanned": "If there are scanned pages, warn me instead of exporting them as images",
    "convert.office.beta": "Beta feature: the layout may differ from the original.",
    "convert.office.run": "Export", "convert.office.label": "Office",
    "convert.office.job": "Export {name} to Office", "convert.office.started": "Exporting to Office… follow it in the task center",
  },
} satisfies Record<"es" | "en", Record<string, string>>;

export const ct = scoped<string>(convertStrings);
