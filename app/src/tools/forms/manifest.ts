import type { ToolManifest } from "../registry";
import { registerTool } from "../registry";
import { registerMode } from "../../shell";
import { registerPageOverlay } from "../../viewer/overlays";
import { FieldsLayer } from "./FieldsLayer";
import { FillToolbar } from "./FillToolbar";
import { ft } from "./text";
import "./strings";

/** Rellenar y firmar (W26): campos sobre el visor y «Mi firma». Sustituye al marcador del modo `forms.fill`. */
export const fill: ToolManifest = {
  id: "forms.fill", group: "fill-sign", kind: "mode", title: "tools.forms.fill.title", description: "tools.forms.fill.description",
  icon: "pen-line", keywords: ["rellenar", "formulario", "firma", "firmar", "campos"], when: { document: true }, pinnable: true,
};

export function registerFormsTools(): void {
  registerTool(fill);
  registerMode({ id: fill.id, get title() { return ft("tools.forms.fill.title"); }, icon: fill.icon, order: 30, needsDocument: true, toolbar: FillToolbar });
  registerPageOverlay(FieldsLayer);
}
