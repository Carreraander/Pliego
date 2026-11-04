import type { ToolManifest } from "../registry";
import { registerTool } from "../registry";
import "./strings";

/** Herramientas de Organizar (W23). Sustituyen a las «fantasma» de `base.ts` con el mismo `id`. */
export const merge: ToolManifest = {
  id: "pages.merge", group: "organize", kind: "task", title: "tools.pages.merge.title", description: "tools.pages.merge.description",
  icon: "combine", keywords: ["unir", "merge", "juntar"], pinnable: true, load: () => import("./MergePanel"),
};
export const split: ToolManifest = {
  id: "pages.split", group: "organize", kind: "task", title: "tools.pages.split.title", description: "tools.pages.split.description",
  icon: "scissors", keywords: ["separar", "split", "partir"], when: { document: true }, pinnable: true, load: () => import("./SplitPanel"),
};

export function registerOrganizeTools(): void {
  registerTool(merge);
  registerTool(split);
}
