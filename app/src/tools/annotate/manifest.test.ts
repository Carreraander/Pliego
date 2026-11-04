import { describe, expect, it } from "vitest";
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
    for (const id of ["annotate.xfdf", "annotate.summary"]) {
      const t = getTool(id)!;
      expect(t.ghost).toBeFalsy();
      expect(typeof t.load).toBe("function");
      expect(toolState(t, none).enabled).toBe(false);
    }
  });
  it("tiene textos en español e inglés", () => {
    for (const k of ["annotate.summary.total", "annotate.xfdf.title", "tools.annotate.summary.title"]) {
      expect(toolText(k, "es")).not.toBe(k);
      expect(toolText(k, "en")).not.toBe(toolText(k, "es"));
    }
  });
});
