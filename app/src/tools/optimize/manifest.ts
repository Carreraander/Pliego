import { registerTool, type ToolManifest } from "../registry";
import { addToolStrings } from "../strings";

export const compress: ToolManifest = {
  id: "optimize.compress", group: "compare-optimize", kind: "task",
  title: "tools.optimize.compress.title", description: "tools.optimize.compress.description",
  icon: "file-down", keywords: ["reducir", "tamaño", "optimizar", "compress", "shrink"], when: { document: true }, pinnable: true,
  load: () => import("./CompressPanel"),
};

export const advanced: ToolManifest = {
  id: "optimize.advanced", group: "compare-optimize", kind: "task",
  title: "tools.optimize.advanced.title", description: "tools.optimize.advanced.description",
  icon: "sliders-horizontal", keywords: ["optimizar", "subconjunto", "fuentes", "resolución", "dpi"], when: { document: true }, pinnable: true,
  load: () => import("./AdvancedPanel"),
};

/** Sustituye a las herramientas fantasma de Comprimir y Optimizar avanzado. */
export function registerOptimizeTools(): void {
  addToolStrings({
    es: { "tools.optimize.compress.title": "Comprimir", "tools.optimize.compress.description": "Reduce el tamaño del archivo", "tools.optimize.advanced.title": "Optimizar avanzado", "tools.optimize.advanced.description": "Controla imágenes, fuentes y estructura" },
    en: { "tools.optimize.compress.title": "Compress", "tools.optimize.compress.description": "Make the file smaller", "tools.optimize.advanced.title": "Advanced optimize", "tools.optimize.advanced.description": "Control images, fonts and structure" },
  });
  registerTool(compress);
  registerTool(advanced);
}
