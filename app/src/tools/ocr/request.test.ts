import { describe, expect, it } from "vitest";
import { defaultLanguages, languageName, normalizePages, ocrArgs, type OcrForm } from "./request";

const form: OcrForm = { path: "C:\\a\\scan.pdf", langs: ["spa", "eng"], pages: "", force: false, deskew: false, rotate: false, binarize: false, denoise: false, dpi: 300, suffix: "OCR" };

describe("OCR", () => {
  it("arma la petición con los nombres que espera el comando", () => {
    expect(ocrArgs(form)).toEqual({ request: { path: "C:\\a\\scan.pdf", langs: "spa+eng", pages: "all", force: false, deskew: false, rotate: false, binarize: false, denoise: false, dpi: 300, suffix: "OCR" } });
  });
  it("rechaza sin idiomas, sin archivo o con páginas mal escritas", () => {
    expect(ocrArgs({ ...form, langs: [] })).toBeNull();
    expect(ocrArgs({ ...form, path: "" })).toBeNull();
    expect(ocrArgs({ ...form, pages: "uno" })).toBeNull();
  });
  it("normaliza páginas", () => {
    expect(normalizePages(" 1 - 3 ")).toBeNull();
    expect(normalizePages("1-3, 5")).toBe("1-3,5");
    expect(normalizePages("Todas")).toBe("all");
  });
  it("elige español e inglés por defecto si están, y si no el primero instalado", () => {
    expect(defaultLanguages(["eng", "fra", "spa"])).toEqual(["spa", "eng"]);
    expect(defaultLanguages(["fra", "deu"])).toEqual(["fra"]);
    expect(defaultLanguages([])).toEqual([]);
  });
  it("da nombre a los idiomas conocidos", () => {
    expect(languageName("spa", true)).toBe("Español (spa)");
    expect(languageName("spa", false)).toBe("Spanish (spa)");
    expect(languageName("xyz", true)).toBe("xyz");
  });
});
