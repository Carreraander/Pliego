import { registerTool, type ToolManifest } from "../registry";
import "./strings";

/** Comparar (W24): sustituye a la «fantasma» de `base.ts`. Es un panel de tarea: guarda un informe PDF con los cambios. */
export const compare: ToolManifest = {
  id: "compare.compare", group: "compare-optimize", kind: "task", title: "tools.compare.compare.title", description: "tools.compare.compare.description",
  icon: "columns-2", keywords: ["comparar", "diferencias", "versiones", "cambios"], when: { document: true }, pinnable: true, load: () => import("./ComparePanel"),
};

export function registerCompareTools(): void {
  registerTool(compare);
}
