import { describe, expect, it } from "vitest";
import { createArgs, createProblem, exportArgs, pdfaArgs, normalizePages, officeArgs, optionalNumber, usesDpi } from "./request";
import { convertStrings } from "./strings";

describe("argumentos de Convertir", () => {
  it("normaliza los rangos de páginas", () => {
    expect(normalizePages("")).toBe("all");
    expect(normalizePages(" Todas ")).toBe("all");
    expect(normalizePages("last")).toBe("last");
    expect(normalizePages("1-3, 5")).toBe("1-3,5");
    expect(normalizePages("a-b")).toBeNull();
    expect(normalizePages("1-")).toBeNull();
  });
  it("lee números opcionales", () => {
    expect(optionalNumber("", 36, 1200)).toBeNull();
    expect(optionalNumber("300", 36, 1200)).toBe(300);
    expect(optionalNumber("5", 36, 1200)).toBeNull();
  });
  it("solo manda resolución y calidad a los formatos que las usan", () => {
    expect(usesDpi("png")).toBe(true);
    expect(usesDpi("txt")).toBe(false);
    expect(exportArgs("a.pdf", "png", "all", 200, 80, "imágenes")).toMatchObject({ dpi: 200, jpegQuality: null });
    expect(exportArgs("a.pdf", "jpg", "all", 200, 80, "imágenes")).toMatchObject({ dpi: 200, jpegQuality: 80 });
    expect(exportArgs("a.pdf", "html", "all", 200, 80, "texto")).toMatchObject({ dpi: null, jpegQuality: null });
    expect(officeArgs("a.pdf", "docx", "1-2", true, "Word")).toEqual({ path: "a.pdf", format: "docx", pages: "1-2", imageDpi: null, failOnScanned: true, password: null, label: "Word", lang: "es" });
  });
  it("los textos están en español e inglés", () => {
    expect(Object.keys(convertStrings.en).sort()).toEqual(Object.keys(convertStrings.es).sort());
  });
  it("Crear PDF: texto y Office van de uno en uno, las imágenes se pueden juntar", () => {
    expect(createProblem([])).toBe("empty");
    expect(createProblem(["a.png", "b.JPG"])).toBeNull();
    expect(createProblem(["nota.txt"])).toBeNull();
    expect(createProblem(["nota.txt", "a.png"])).toBe("single");
    expect(createProblem(["C:\\x\\informe.DOCX", "b.docx"])).toBe("single");
    expect(createProblem(["libro.epub"])).toBeNull();
    expect(createProblem(["tebeo.CBZ", "a.png"])).toBe("single");
    expect(createProblem(["doc.xps", "b.xps"])).toBe("single");
    expect(createArgs(["a.png"], "a4", null, "auto", false, "PDF")).toEqual({ paths: ["a.png"], pageSize: "a4", marginMm: 0, orientation: "auto", upscale: false, label: "PDF" });
    expect(pdfaArgs("a.pdf", "2b", "PDF-A")).toEqual({ path: "a.pdf", level: "2b", label: "PDF-A" });
  });
});
