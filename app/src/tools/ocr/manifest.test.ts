import { describe, expect, it } from "vitest";
import { getTool, toolState } from "../registry";
import { registerBaseTools } from "../base";
import { registerOcrTools } from "./manifest";
import { registerCompareTools } from "../compare/manifest";

const none = { document: false, forms: false, signatures: false, protected: false, pdfa: false };
const open = { ...none, document: true };

describe("OCR y Comparar (W24)", () => {
  it("sustituyen a las herramientas fantasma", () => {
    registerBaseTools();
    for (const id of ["ocr.recognize", "ocr.clean", "ocr.languages", "compare.compare"]) expect(getTool(id)?.ghost).toBe(true);
    registerOcrTools();
    registerCompareTools();
    for (const id of ["ocr.recognize", "ocr.clean", "ocr.languages", "compare.compare"]) {
      const t = getTool(id)!;
      expect(t.ghost, id).toBeFalsy();
      expect(typeof t.load, id).toBe("function");
    }
  });
  it("reconocer, limpiar y comparar piden un documento; los idiomas no", () => {
    registerBaseTools();
    registerOcrTools();
    registerCompareTools();
    for (const id of ["ocr.recognize", "ocr.clean", "compare.compare"]) {
      expect(toolState(getTool(id)!, none).enabled, id).toBe(false);
      expect(toolState(getTool(id)!, open).enabled, id).toBe(true);
    }
    expect(toolState(getTool("ocr.languages")!, none).enabled).toBe(true);
  });
});
