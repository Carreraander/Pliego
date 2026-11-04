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

/** Sustituye a las fantasma de Exportar a imágenes o texto y Exportar a Office. */
export function registerConvertTools(): void {
  addToolStrings({
    es: { "tools.convert.export.title": "Exportar a imágenes o texto", "tools.convert.export.description": "Guarda las páginas como imágenes o texto", "tools.convert.office.title": "Exportar a Word, Excel o PowerPoint", "tools.convert.office.description": "Convierte a formatos de Office (beta)" },
    en: { "tools.convert.export.title": "Export to images or text", "tools.convert.export.description": "Save pages as images or text", "tools.convert.office.title": "Export to Word, Excel or PowerPoint", "tools.convert.office.description": "Convert to Office formats (beta)" },
  });
  registerTool(exportImages);
  registerTool(office);
}
