import { beforeEach, describe, expect, it } from "vitest";
import { setLocale } from "../../i18n";
import { registerBaseTools } from "../base";
import { getTool, NO_DOCUMENT, setDocFacts, toolState } from "../registry";
import { registerConvertTools } from "./manifest";

beforeEach(() => { setLocale("es"); setDocFacts(NO_DOCUMENT); registerBaseTools(); });

describe("Convertir", () => {
  it("las herramientas reales sustituyen a las fantasma", () => {
    expect(getTool("convert.export")?.ghost).toBe(true);
    registerConvertTools();
    for (const id of ["convert.export", "convert.office", "convert.create", "convert.pdfa"]) {
      expect(getTool(id)?.ghost).toBeUndefined();
      if (id !== "convert.create") expect(toolState(getTool(id)!).enabled).toBe(false);
    }
    expect(toolState(getTool("convert.create")!).enabled).toBe(true); // no necesita documento
    setDocFacts({ document: true });
    expect(toolState(getTool("convert.office")!).enabled).toBe(true);
  });
});
