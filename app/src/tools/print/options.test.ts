import { describe, expect, it } from "vitest";
import {
  defaultOptions, isMultiUp, landscapeSheet, loadRemembered, pagesExpression, paperSizeMm, rangeError, remember, selectedPages,
  sheetLayout, thumbZoom, withSheetLayout,
} from "./options";

describe("imprimir: páginas", () => {
  it("arma la expresión según la elección", () => {
    expect(pagesExpression("all", 3, "")).toBe("all");
    expect(pagesExpression("current", 3, "")).toBe("3");
    expect(pagesExpression("range", 3, " 1-2, 5 ")).toBe("1-2, 5");
  });

  it("valida el rango contra el número de páginas", () => {
    expect(rangeError("1-3,5", 10)).toBeNull();
    expect(rangeError("7-", 10)).toBeNull();
    expect(rangeError("", 10)).toBe("empty");
    expect(rangeError("a-b", 10)).toBe("syntax");
    expect(rangeError("1-3x", 10)).toBe("syntax");
    expect(rangeError("11", 10)).toBe("outside");
    expect(rangeError("0", 10)).toBe("outside");
    expect(rangeError("3-12", 10)).toBe("outside");
  });

  it("calcula las páginas que salen (igual que el motor)", () => {
    expect(selectedPages("all", "all", false, 4)).toEqual([0, 1, 2, 3]);
    expect(selectedPages("2-3", "all", false, 4)).toEqual([1, 2]);
    expect(selectedPages("1,3-4", "all", false, 5)).toEqual([0, 2, 3]);
    expect(selectedPages("all", "odd", false, 5)).toEqual([0, 2, 4]);
    expect(selectedPages("2-5", "even", false, 6)).toEqual([1, 3]);
    expect(selectedPages("1-3", "all", true, 3)).toEqual([2, 1, 0]);
    expect(selectedPages("3-", "all", false, 5)).toEqual([2, 3, 4]);
    expect(selectedPages("2-2,2", "all", false, 5)).toEqual([1]);
  });
});

describe("imprimir: hoja", () => {
  it("elige la orientación de la hoja", () => {
    expect(landscapeSheet("auto", { width: 842, height: 595 })).toBe(true);
    expect(landscapeSheet("auto", { width: 595, height: 842 })).toBe(false);
    expect(landscapeSheet("auto", undefined)).toBe(false);
    expect(landscapeSheet("portrait", { width: 842, height: 595 })).toBe(false);
    expect(landscapeSheet("landscape", { width: 595, height: 842 })).toBe(true);
  });

  it("da el papel en la orientación pedida", () => {
    expect(paperSizeMm(210, 297, false)).toEqual([210, 297]);
    expect(paperSizeMm(210, 297, true)).toEqual([297, 210]);
    expect(paperSizeMm(297, 210, false)).toEqual([210, 297]);
  });

  it("la miniatura cabe en una tesela", () => {
    expect(thumbZoom(595, 842) * 842).toBeLessThanOrEqual(512);
    expect(thumbZoom(100, 100)).toBe(1);
  });
});

describe("imprimir: lo recordado", () => {
  const fake = () => {
    const m = new Map<string, string>();
    return { getItem: (k: string) => m.get(k) ?? null, setItem: (k: string, v: string) => void m.set(k, v) };
  };

  it("recuerda ajustes pero no páginas ni copias", () => {
    const s = fake();
    remember({ ...defaultOptions(), printer: "HP", scale: "actual", copies: 5, pages: "1-2", grayscale: true }, s);
    expect(loadRemembered(s)).toEqual({
      printer: "HP", orientation: "auto", autoRotate: true, scale: "actual", scalePercent: 100, duplex: "off", grayscale: true,
      comments: true, collate: true, nupOrder: "horizontal", borders: false,
    });
  });

  it("tolera un almacenamiento roto o ausente", () => {
    expect(loadRemembered({ getItem: () => "{no es json" })).toEqual({});
    expect(loadRemembered({ getItem: () => { throw new Error("bloqueado"); } })).toEqual({});
    expect(() => remember(defaultOptions(), { setItem: () => { throw new Error("lleno"); } })).not.toThrow();
  });
});

describe("imprimir: reparto en las hojas", () => {
  it("traduce el reparto elegido a opciones y vuelta", () => {
    const base = defaultOptions();
    expect(sheetLayout(base)).toBe("1");
    const four = { ...base, ...withSheetLayout("4") };
    expect(four).toMatchObject({ pagesPerSheet: 4, booklet: "off" });
    expect(sheetLayout(four)).toBe("4");
    const booklet = { ...four, subset: "odd" as const, ...withSheetLayout("booklet") };
    expect(booklet).toMatchObject({ pagesPerSheet: 1, booklet: "both", subset: "all" });
    expect(sheetLayout(booklet)).toBe("booklet");
    expect(withSheetLayout("booklet", "frontOnly").booklet).toBe("frontOnly");
  });

  it("solo con varias por hoja o folleto se deja de elegir la escala", () => {
    expect(isMultiUp(defaultOptions())).toBe(false);
    expect(isMultiUp({ ...defaultOptions(), pagesPerSheet: 2 })).toBe(true);
    expect(isMultiUp({ ...defaultOptions(), booklet: "backOnly" })).toBe(true);
  });
});
