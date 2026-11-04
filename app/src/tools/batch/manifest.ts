import type { ToolManifest } from "../registry";
import { registerTool } from "../registry";
import "./strings";

/** Herramientas de proceso por lotes (W29). Sustituyen a las «fantasma» de `base.ts` con el mismo `id`. */
export const indexFolder: ToolManifest = {
  id: "batch.index", group: "automate", kind: "wizard", title: "tools.batch.index.title", description: "tools.batch.index.description",
  icon: "database", keywords: ["índice", "indexar", "buscar", "carpeta"], pinnable: true, load: () => import("./IndexPanel"),
};

export function registerBatchTools(): void {
  registerTool(indexFolder);
}
