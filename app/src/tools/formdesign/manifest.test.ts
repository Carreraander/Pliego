import { describe, expect, it, vi } from "vitest";

// La capa del visor necesitan el DOM; aquí solo se comprueba el registro de la herramienta.
vi.mock("../../viewer/overlays", () => ({ registerPageOverlay: vi.fn() }));
vi.mock("./FormDesignLayer", () => ({ FormDesignLayer: () => null }));
import { getTool, toolState } from "../registry";
import { registerBaseTools } from "../base";
import { registerFormTools } from "./manifest";
import { toolText } from "../strings";

describe("Diseñar formulario", () => {
  it("sustituye a la fantasma de base.ts y pide un documento", () => {
    registerBaseTools();
    expect(getTool("forms.design")?.ghost).toBe(true);
    registerFormTools();
    const t = getTool("forms.design")!;
    expect(t.ghost).toBeFalsy();
    expect(toolState(t, { document: false, forms: false, signatures: false, protected: false, pdfa: false }).enabled).toBe(false);
    expect(typeof t.load).toBe("function");
  });
  it("tiene textos en español e inglés", () => {
    for (const k of ["fdesign.panel.create", "fdesign.err.nameDot", "tools.forms.design.title", "fdesign.tool.signature"]) {
      expect(toolText(k, "es")).not.toBe(k);
      expect(toolText(k, "en")).not.toBe(toolText(k, "es"));
    }
  });
});
