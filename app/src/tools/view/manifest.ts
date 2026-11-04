import type { ToolManifest } from "../registry";
import { registerTool } from "../registry";
import "./strings";

/** Herramientas de Ver (W29). Sustituyen a las «fantasma» de `base.ts` con el mismo `id`. */
export const properties: ToolManifest = {
  id: "viewer.properties", group: "view", kind: "task", title: "tools.viewer.properties.title", description: "tools.viewer.properties.description",
  icon: "info", keywords: ["metadatos", "autor", "info", "propiedades"], shortcut: "Ctrl+D", when: { document: true }, pinnable: true,
  load: () => import("./PropertiesPanel"),
};

export function registerViewTools(): void {
  registerTool(properties);
}
