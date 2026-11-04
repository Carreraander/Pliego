import { describe, expect, it, vi } from "vitest";

// El modo y la capa del visor necesitan el DOM; aquí solo se comprueba el registro de herramientas.
const { registerMode } = vi.hoisted(() => ({ registerMode: vi.fn() }));
vi.mock("../../shell", () => ({ registerMode, activeMode: () => null }));
vi.mock("../../viewer/overlays", () => ({ registerPageOverlay: vi.fn() }));
vi.mock("./CommentLayer", () => ({ CommentLayer: () => null }));
vi.mock("./CommentToolbar", () => ({ CommentToolbar: () => null }));
import { getTool, toolState } from "../registry";
import { registerBaseTools } from "../base";
import { registerAnnotateTools } from "./manifest";
import { toolText } from "../strings";

describe("herramientas de Comentar", () => {
  it("sustituyen a las fantasma de base.ts y piden un documento", () => {
    registerBaseTools();
    expect(getTool("annotate.xfdf")?.ghost).toBe(true);
    registerAnnotateTools();
    const none = { document: false, forms: false, signatures: false, protected: false, pdfa: false };
    for (const id of ["annotate.comment", "annotate.xfdf", "annotate.summary"]) {
      const t = getTool(id)!;
      expect(t.ghost).toBeFalsy();
      if (t.kind !== "mode") expect(typeof t.load).toBe("function");
      expect(toolState(t, none).enabled).toBe(false);
    }
  });
  it("registra el modo Comentar con el mismo id que su herramienta", () => {
    registerAnnotateTools();
    expect(registerMode).toHaveBeenCalledWith(expect.objectContaining({ id: "annotate.comment", needsDocument: true }));
    expect(getTool("annotate.comment")?.kind).toBe("mode");
  });
  it("tiene textos en español e inglés", () => {
    for (const k of ["annotate.summary.total", "annotate.xfdf.title", "tools.annotate.summary.title"]) {
      expect(toolText(k, "es")).not.toBe(k);
      expect(toolText(k, "en")).not.toBe(toolText(k, "es"));
    }
  });
});
