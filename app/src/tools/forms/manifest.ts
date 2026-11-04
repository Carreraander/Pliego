import type { ToolManifest } from "../registry";
import { registerTool } from "../registry";
import { registerMode } from "../../shell";
import { registerPageOverlay } from "../../viewer/overlays";
import { FormDesignLayer } from "./FormDesignLayer";
import { FormDesignToolbar } from "./FormDesignToolbar";
import { ft } from "./text";
import "./strings";

/** Diseñador de formularios (W29b). Sustituye a la herramienta «fantasma» `forms.design` de `base.ts`. */
export const design: ToolManifest = {
  id: "forms.design", group: "forms", kind: "mode", title: "tools.forms.design.title", description: "tools.forms.design.description",
  icon: "pencil", keywords: ["campos", "formulario", "diseñador", "casilla", "desplegable"], when: { document: true }, pinnable: true,
};

export function registerFormTools(): void {
  registerTool(design);
  registerMode({ id: design.id, get title() { return ft("tools.forms.design.title"); }, icon: design.icon, order: 55, needsDocument: true, toolbar: FormDesignToolbar });
  registerPageOverlay(FormDesignLayer);
}
