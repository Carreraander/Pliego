import { describe, expect, it } from "vitest";
import { fromA11y, fromPdfa, fromPreflight } from "./report";

describe("informes de comprobación", () => {
  it("accesibilidad: errores y avisos", () => {
    const s = fromA11y({ pages: 2, tagged: false, language: null, title: null, issues: [
      { severity: "error", page: null, message: "Sin etiquetas", suggestion: "Etiquetar" },
      { severity: "warning", page: 1, message: "Sin marcadores", suggestion: "" },
    ] });
    expect([s.errors, s.warnings]).toEqual([1, 1]);
    expect(s.rows[0]).toMatchObject({ tone: "danger", hint: "Etiquetar" });
  });
  it("PDF/A: todo es error", () => {
    const s = fromPdfa({ level: "2b", issues: [{ page: 0, message: "Fuente no incrustada" }] });
    expect(s).toMatchObject({ errors: 1, warnings: 0 });
  });
  it("comprobación previa: la información no cuenta", () => {
    const s = fromPreflight({ profile: "print", pages: 1, findings: [
      { severity: "info", page: null, message: "a" }, { severity: "warning", page: 0, message: "b" }, { severity: "error", page: 0, message: "c" },
    ] });
    expect([s.errors, s.warnings, s.rows.length]).toEqual([1, 1, 3]);
  });
});
