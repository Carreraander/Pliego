import { registerTool, type ToolManifest } from "../registry";
import "./strings";

/** Reconocer texto (OCR), Enderezar y limpiar e Idiomas de OCR (W24): sustituyen a las «fantasma» de `base.ts`. */
export const recognize: ToolManifest = {
  id: "ocr.recognize", group: "scan-ocr", kind: "task", title: "tools.ocr.recognize.title", description: "tools.ocr.recognize.description",
  icon: "ocr", keywords: ["ocr", "escaneo", "texto buscable", "reconocer"], when: { document: true }, pinnable: true, load: () => import("./RecognizePanel"),
};
export const clean: ToolManifest = {
  id: "ocr.clean", group: "scan-ocr", kind: "task", title: "tools.ocr.clean.title", description: "tools.ocr.clean.description",
  icon: "wand", keywords: ["enderezar", "limpiar", "ruido", "escaneo", "deskew"], when: { document: true }, pinnable: true, load: () => import("./CleanPanel"),
};
export const languages: ToolManifest = {
  id: "ocr.languages", group: "scan-ocr", kind: "task", title: "tools.ocr.languages.title", description: "tools.ocr.languages.description",
  icon: "languages", keywords: ["idiomas", "ocr", "modelos", "descargar"], pinnable: true, load: () => import("./LanguagesPanel"),
};

export function registerOcrTools(): void {
  registerTool(recognize);
  registerTool(clean);
  registerTool(languages);
}
