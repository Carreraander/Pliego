import { describe, expect, it } from "vitest";
import { addBox, blockAt, parseSize, styleFor, type TextBlock } from "./geometry";

const block = (id: number, bbox: [number, number, number, number]): TextBlock =>
  ({ page: 0, id, bbox, text: "t", lines: ["t"], font_size: 12, bold: false, italic: false, family: "sans" });

describe("geometría de la edición de texto", () => {
  it("un clic da un recuadro por defecto y un arrastre el arrastrado", () => {
    expect(addBox([10, 20], [11, 21])).toEqual([10, 20, 210, 60]);
    expect(addBox([100, 90], [20, 30])).toEqual([20, 30, 100, 90]);
  });
  it("el estilo solo lleva lo que cambió respecto al bloque", () => {
    const b = block(0, [0, 0, 1, 1]);
    expect(styleFor(b, { family: "sans", size: 12, bold: false, italic: false, align: "left" })).toEqual({ align: "left" });
    expect(styleFor(b, { family: "serif", size: 14, bold: true, italic: false, align: "center" })).toEqual({ align: "center", family: "serif", size: 14, bold: true });
    expect(styleFor(null, { family: "sans", size: 12, bold: false, italic: false, align: "left" })).toMatchObject({ family: "sans", size: 12 });
  });
  it("valida el tamaño de letra", () => {
    expect(parseSize("10,5")).toBe(10.5);
    expect(parseSize("0")).toBeNull();
    expect(parseSize("abc")).toBeNull();
    expect(parseSize("500")).toBeNull();
  });
  it("encuentra el bloque bajo el punto, el más pequeño si se solapan", () => {
    const big = block(1, [0, 0, 200, 200]);
    const small = block(2, [10, 10, 50, 30]);
    expect(blockAt([big, small], [20, 20])?.id).toBe(2);
    expect(blockAt([big, small], [150, 150])?.id).toBe(1);
    expect(blockAt([big, small], [300, 300])).toBeNull();
  });
});
