import type { ToolManifest } from "../registry";
import { registerTool } from "../registry";
import { registerPageOverlay } from "../../viewer/overlays";
import { MeasureLayer } from "./MeasureLayer";
import "./strings";

/** Medir (W29). Sustituye a la herramienta «fantasma» `batch.measure` de `base.ts`. */
export const measure: ToolManifest = {
  id: "batch.measure", group: "automate", kind: "task", title: "tools.batch.measure.title", description: "tools.batch.measure.description",
  icon: "ruler", keywords: ["medidas", "planos", "escala", "distancia", "área"], when: { document: true }, pinnable: true,
  load: () => import("./MeasurePanel"),
};

export function registerMeasureTools(): void {
  registerTool(measure);
  registerPageOverlay(MeasureLayer);
}
