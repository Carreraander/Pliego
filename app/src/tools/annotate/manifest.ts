import type { ToolManifest } from "../registry";
import { registerTool } from "../registry";
import "./strings";

/** Herramientas de Comentar (W26). Sustituyen a las «fantasma» de `base.ts` con el mismo `id`. */
export const xfdf: ToolManifest = {
  id: "annotate.xfdf", group: "comment", kind: "task", title: "tools.annotate.xfdf.title", description: "tools.annotate.xfdf.description",
  icon: "arrow-left-right", keywords: ["xfdf", "comentarios", "importar", "exportar"], when: { document: true }, pinnable: true,
  load: () => import("./XfdfPanel"),
};
export const summary: ToolManifest = {
  id: "annotate.summary", group: "comment", kind: "task", title: "tools.annotate.summary.title", description: "tools.annotate.summary.description",
  icon: "list", keywords: ["resumen", "comentarios", "lista"], when: { document: true }, pinnable: true,
  load: () => import("./SummaryPanel"),
};

export function registerAnnotateTools(): void {
  registerTool(xfdf);
  registerTool(summary);
}
