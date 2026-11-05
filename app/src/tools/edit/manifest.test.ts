import { describe, expect, it, vi } from "vitest";

// El modo y la capa del visor necesitan el DOM; aquí solo se comprueba el registro de la herramienta.
const { registerMode } = vi.hoisted(() => ({ registerMode: vi.fn() }));
vi.mock("../../shell", () => ({ registerMode, activeMode: () => null }));
vi.mock("../../viewer/overlays", () => ({ registerPageOverlay: vi.fn() }));
vi.mock("./EditLayer", () => ({ EditLayer: () => null }));
vi.mock("./EditToolbar", () => ({ EditToolbar: () => null }));
import { getTool, toolState } from "../registry";
import { registerBaseTools } from "../base";
import { registerEditTools } from "./manifest";
import { toolText } from "../strings";

describe("Editar", () => {
  it("sustituye a la fantasma de base.ts, pide un documento y registra el quinto modo", () => {
    registerBaseTools();
    expect(getTool("edit.content")?.ghost).toBe(true);
    registerEditTools();
    const t = getTool("edit.content")!;
    expect(t.ghost).toBeFalsy();
    expect(toolState(t, { document: false, forms: false, signatures: false, protected: false, pdfa: false }).enabled).toBe(false);
    expect(registerMode).toHaveBeenCalledWith(expect.objectContaining({ id: "edit.content", order: 50, needsDocument: true }));
  });
  it("tiene textos en español e inglés", () => {
    for (const k of ["edit.editor.apply", "edit.hint.add", "tools.edit.content.title", "edit.family.mono", "edit.tool.image", "edit.hint.link", "edit.reflow", "edit.link.bad"]) {
      expect(toolText(k, "es")).not.toBe(k);
      expect(toolText(k, "en")).not.toBe(toolText(k, "es"));
    }
  });
});
