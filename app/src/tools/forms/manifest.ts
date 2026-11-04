import type { ToolManifest } from "../registry";
import { registerTool } from "../registry";
import { registerPageOverlay } from "../../viewer/overlays";
import { FormDesignLayer } from "./FormDesignLayer";
import "./strings";

/** Diseñador de formularios (W29b). Sustituye a la herramienta «fantasma» `forms.design` de `base.ts`. */
export const design: ToolManifest = {
  id: "forms.design", group: "forms", kind: "task", title: "tools.forms.design.title", description: "tools.forms.design.description",
  icon: "pencil", keywords: ["campos", "formulario", "diseñador", "casilla", "desplegable"], when: { document: true }, pinnable: true,
  load: () => import("./DesignPanel"),
};

export function registerFormTools(): void {
  registerTool(design);
  registerPageOverlay(FormDesignLayer);
}
