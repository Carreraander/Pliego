import { describe, expect, it, vi } from "vitest";
import { getTool, toolState } from "../registry";
import { registerBaseTools } from "../base";
import { toolText } from "../strings";

// El modo y las capas necesitan el DOM; aquí solo se comprueba el registro de la herramienta.
const { registerMode } = vi.hoisted(() => ({ registerMode: vi.fn() }));
vi.mock("../../shell", () => ({ registerMode, activeMode: () => null }));
vi.mock("../../viewer/overlays", () => ({ registerPageOverlay: vi.fn() }));
vi.mock("./FieldsLayer", () => ({ FieldsLayer: () => null }));
vi.mock("./FillToolbar", () => ({ FillToolbar: () => null }));
const { registerFormsTools } = await import("./manifest");

describe("herramienta Rellenar y firmar", () => {
  it("sustituye a la fantasma, registra su modo y pide un documento", () => {
    registerBaseTools();
    expect(getTool("forms.fill")?.ghost).toBe(true);
    registerFormsTools();
    const t = getTool("forms.fill")!;
    expect(t.ghost).toBeFalsy();
    expect(registerMode).toHaveBeenCalledWith(expect.objectContaining({ id: "forms.fill", needsDocument: true }));
    const none = { document: false, forms: false, signatures: false, protected: false, pdfa: false };
    expect(toolState(t, none).enabled).toBe(false);
  });
  it("tiene textos en español e inglés", () => {
    for (const k of ["tools.forms.fill.title", "forms.mySignature", "forms.signature.save"]) {
      expect(toolText(k, "es")).not.toBe(k);
      expect(toolText(k, "en")).not.toBe(toolText(k, "es"));
    }
  });
});
