import { describe, expect, it } from "vitest";
import { defaultLanguage, figuresWithoutAlt, fixOptions, type FixForm } from "./fix";
import type { A11yReport } from "./report";

const report = (over: Partial<A11yReport> = {}): A11yReport => ({ pages: 2, tagged: false, language: null, title: null, issues: [], ...over });
const form = (over: Partial<FixForm> = {}): FixForm => ({ tagged: false, autotag: true, title: "", language: "es-ES", tabOrder: true, alts: {}, ...over });

describe("Hacer accesible", () => {
  it("en un documento sin etiquetas etiqueta, pone título e idioma y la tabulación", () => {
    expect(fixOptions(form({ title: " Acta " }), report())).toEqual({ autotag: true, title: "Acta", language: "es-ES", alt_texts: [], tab_order: true });
  });

  it("no manda lo que ya está igual ni etiqueta dos veces", () => {
    const r = report({ tagged: true, title: "Acta", language: "es-ES" });
    expect(fixOptions(form({ tagged: true, title: "Acta", tabOrder: false }), r)).toBeNull();
    expect(fixOptions(form({ tagged: true, title: "Acta", tabOrder: false, alts: { 12: " Logo ", 13: "  " } }), r))
      .toEqual({ autotag: false, title: null, language: null, alt_texts: [[12, "Logo"]], tab_order: false });
  });

  it("sin etiquetas ni autoetiquetado la tabulación no sirve de nada", () => {
    expect(fixOptions(form({ autotag: false, language: "" }), report())).toBeNull();
  });

  it("saca las figuras sin texto alternativo y propone el idioma", () => {
    const r = report({ tagged: true, issues: [
      { code: "figure_without_alt", severity: "error", page: 1, xref: 40, message: "", suggestion: "" },
      { code: "no_title", severity: "error", page: null, xref: null, message: "", suggestion: "" },
    ] });
    expect(figuresWithoutAlt(r)).toEqual([{ xref: 40, page: 1 }]);
    expect(defaultLanguage(r, "en")).toBe("en-US");
    expect(defaultLanguage(report({ language: "ca" }), "es")).toBe("ca");
  });
});
