import { describe, expect, it, vi } from "vitest";

// La capa del visor necesita el DOM; aquí solo se comprueba el registro de la herramienta.
vi.mock("../../viewer/overlays", () => ({ registerPageOverlay: vi.fn() }));
vi.mock("./MeasureLayer", () => ({ MeasureLayer: () => null }));

import { getTool, toolState } from "../registry";
import { registerBaseTools } from "../base";
import { registerMeasureTools } from "./manifest";

describe("medir (W29)", () => {
  it("sustituye a la fantasma de base.ts y pide un documento", () => {
    registerBaseTools();
    expect(getTool("batch.measure")?.ghost).toBe(true);
    registerMeasureTools();
    const t = getTool("batch.measure")!;
    expect(t.ghost).toBeFalsy();
    expect(t.kind).toBe("task");
    const none = { document: false, forms: false, signatures: false, protected: false, pdfa: false };
    expect(toolState(t, none).enabled).toBe(false);
  });
});
