import { registerTool, type ToolManifest } from "../registry";
import { addToolStrings } from "../strings";

export const exportImages: ToolManifest = {
  id: "convert.export", group: "convert", kind: "task",
  title: "tools.convert.export.title", description: "tools.convert.export.description",
  icon: "file-output", keywords: ["exportar", "png", "jpg", "txt", "html", "imágenes", "texto"], when: { document: true }, pinnable: true,
  load: () => import("./ExportPanel"),
};

export const office: ToolManifest = {
  id: "convert.office", group: "convert", kind: "task",
  title: "tools.convert.office.title", description: "tools.convert.office.description",
  icon: "file-type", keywords: ["word", "excel", "powerpoint", "office", "docx", "xlsx", "pptx"], when: { document: true }, pinnable: true,
  load: () => import("./OfficePanel"),
};

export const create: ToolManifest = {
  id: "convert.create", group: "convert", kind: "task",
  title: "tools.convert.create.title", description: "tools.convert.create.description",
  icon: "file-plus-2", keywords: ["crear", "imágenes", "convertir", "texto", "word"], pinnable: true,
  load: () => import("./CreatePanel"),
};

export const pdfa: ToolManifest = {
  id: "convert.pdfa", group: "convert", kind: "task",
  title: "tools.convert.pdfa.title", description: "tools.convert.pdfa.description",
  icon: "pdf-a", keywords: ["pdf/a", "archivo", "conservación"], when: { document: true }, pinnable: true,
  load: () => import("./PdfaPanel"),
};

/** Sustituye a las fantasma de Exportar a imágenes o texto y Exportar a Office. */
export function registerConvertTools(): void {
  addToolStrings({
    es: { "tools.convert.export.title": "Exportar a imágenes o texto", "tools.convert.export.description": "Guarda las páginas como imágenes o texto", "tools.convert.office.title": "Exportar a Word, Excel o PowerPoint", "tools.convert.office.description": "Convierte a formatos de Office (beta)", "tools.convert.create.title": "Crear PDF", "tools.convert.create.description": "Crea un PDF desde imágenes u otros archivos", "tools.convert.pdfa.title": "Guardar como PDF/A", "tools.convert.pdfa.description": "Archivo de larga conservación" },
    en: { "tools.convert.export.title": "Export to images or text", "tools.convert.export.description": "Save pages as images or text", "tools.convert.office.title": "Export to Word, Excel or PowerPoint", "tools.convert.office.description": "Convert to Office formats (beta)", "tools.convert.create.title": "Create PDF", "tools.convert.create.description": "Create a PDF from images or other files", "tools.convert.pdfa.title": "Save as PDF/A", "tools.convert.pdfa.description": "Long-term archive format" },
  });
  registerTool(exportImages);
  registerTool(office);
  registerTool(create);
  registerTool(pdfa);
}
