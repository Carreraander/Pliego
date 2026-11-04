import { describe, expect, it } from "vitest";
import { formatBytes } from "../format";
import { compressArgs, parseOptional, QUALITY_PRESET } from "./request";

describe("argumentos de Comprimir", () => {
  it("cada calidad corresponde a un preajuste de pliego-optimize", () => {
    expect(QUALITY_PRESET).toEqual({ max: "impresion", balanced: "ebook", min: "pantalla" });
  });
  it("manda nulos cuando no hay ajustes avanzados", () => {
    expect(compressArgs("C:\\a.pdf", "ebook", "comprimido")).toEqual({
      path: "C:\\a.pdf", preset: "ebook", dpi: null, jpegQuality: null, password: null, suffix: "comprimido",
    });
    expect(compressArgs("a.pdf", "pantalla", "x", 110, 60)).toMatchObject({ dpi: 110, jpegQuality: 60 });
  });
  it("los campos vacíos o fuera de rango usan el del preajuste", () => {
    expect(parseOptional("", 36, 1200)).toBeNull();
    expect(parseOptional("abc", 36, 1200)).toBeNull();
    expect(parseOptional("10", 36, 1200)).toBeNull();
    expect(parseOptional(" 150 ", 36, 1200)).toBe(150);
    expect(parseOptional("72,5", 36, 1200)).toBe(72.5);
  });
  it("da el tamaño legible", () => {
    expect(formatBytes(900)).toBe("900 B");
    expect(formatBytes(1536)).toBe("1,5 KB");
    expect(formatBytes(3 * 1024 * 1024, false)).toBe("3.0 MB");
  });
});
