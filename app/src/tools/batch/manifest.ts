import type { ToolManifest } from "../registry";
import { registerTool } from "../registry";
import "./strings";

/** Herramientas de proceso por lotes (W29). Sustituyen a las «fantasma» de `base.ts` con el mismo `id`. */
export const indexFolder: ToolManifest = {
  id: "batch.index", group: "automate", kind: "wizard", title: "tools.batch.index.title", description: "tools.batch.index.description",
  icon: "database", keywords: ["índice", "indexar", "buscar", "carpeta"], pinnable: true, load: () => import("./IndexPanel"),
};

export const portfolio: ToolManifest = {
  id: "batch.portfolio", group: "automate", kind: "task", title: "tools.batch.portfolio.title", description: "tools.batch.portfolio.description",
  icon: "folders", keywords: ["cartera", "portfolio", "adjuntos", "incrustados"], pinnable: true, load: () => import("./PortfolioPanel"),
};

export const actions: ToolManifest = {
  id: "batch.actions", group: "automate", kind: "wizard", title: "tools.batch.actions.title", description: "tools.batch.actions.description",
  icon: "list-checks", keywords: ["lote", "batch", "automatizar", "acción"], pinnable: true, load: () => import("./ActionsPanel"),
};

export function registerBatchTools(): void {
  registerTool(actions);
  registerTool(indexFolder);
  registerTool(portfolio);
}
