import { describe, expect, it } from "vitest";
import { getTool, toolState } from "../registry";
import { registerBaseTools } from "../base";
import { registerStampTools } from "./manifest";

const IDS = ["stamp.stamps", "edit.watermark", "edit.headerfooter", "edit.background", "edit.bates"];

describe("herramientas de sellado", () => {
  it("sustituyen a las fantasma de base.ts", () => {
    registerBaseTools();
    for (const id of IDS) expect(getTool(id)?.ghost).toBe(true);
    registerStampTools();
    for (const id of IDS) {
      const t = getTool(id)!;
      expect(t.ghost).toBeFalsy();
      expect(typeof t.load).toBe("function");
    }
  });
  it("piden un documento abierto", () => {
    registerStampTools();
    const none = { document: false, forms: false, signatures: false, protected: false, pdfa: false };
    for (const id of IDS) expect(toolState(getTool(id)!, none).enabled).toBe(false);
  });
});
