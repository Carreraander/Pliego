import { describe, expect, it } from "vitest";
import { addBox, blockAt, changed, clampBox, dragBox, parseColor, parseSize, parseTarget, placeForm, rotateTransform, styleFor, targetText, toHex, transformBetween, type TextBlock } from "./geometry";

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

describe("geometría de imágenes, objetos y enlaces", () => {
  const box: [number, number, number, number] = [100, 100, 200, 150];
  it("mover desplaza la caja entera", () => {
    expect(dragBox(box, "move", [0, 0], [10, -5])).toEqual([110, 95, 210, 145]);
  });
  it("un tirador de esquina mueve esa esquina y respeta el lado mínimo", () => {
    expect(dragBox(box, "se", [200, 150], [260, 190])).toEqual([100, 100, 260, 190]);
    expect(dragBox(box, "nw", [100, 100], [300, 300])).toEqual([196, 146, 200, 150]);
    expect(dragBox(box, "e", [200, 125], [230, 999])).toEqual([100, 100, 230, 150]);
  });
  it("la transformación lleva una caja a otra (escala respecto al centro y desplazamiento)", () => {
    const t = transformBetween(box, [100, 100, 300, 150]);
    expect(t).toEqual({ dx: 50, dy: 0, scale_x: 2, scale_y: 1, rotate_degrees: 0 });
    expect(transformBetween(box, box)).toEqual({ dx: 0, dy: 0, scale_x: 1, scale_y: 1, rotate_degrees: 0 });
    expect(rotateTransform(90).rotate_degrees).toBe(90);
  });
  it("distingue un cambio real de un temblor", () => {
    expect(changed(box, [100.2, 100, 200, 150])).toBe(false);
    expect(changed(box, [101, 100, 200, 150])).toBe(true);
  });
  it("el recorte se limita a la imagen y desaparece si queda fuera", () => {
    expect(clampBox([50, 50, 150, 120], box)).toEqual([100, 100, 150, 120]);
    expect(clampBox([300, 300, 400, 400], box)).toBeNull();
  });
  it("convierte colores a componentes y de vuelta", () => {
    expect(parseColor("#ff0000")).toEqual([1, 0, 0]);
    expect(parseColor("rojo")).toBeNull();
    expect(toHex([1, 0.5, 0])).toBe("#ff8000");
  });
  it("el color solo viaja si cambió respecto al bloque", () => {
    const b: TextBlock = { page: 0, id: 0, bbox: [0, 0, 1, 1], text: "t", lines: ["t"], font_size: 12, bold: false, italic: false, family: "sans", color: [0, 0, 0] };
    const v = { family: "sans" as const, size: 12, bold: false, italic: false, align: "left" as const };
    expect(styleFor(b, { ...v, color: [0, 0, 0] })).toEqual({ align: "left" });
    expect(styleFor(b, { ...v, color: [1, 0, 0] })).toEqual({ align: "left", color: [1, 0, 0] });
  });
  it("interpreta el destino de un enlace", () => {
    expect(parseTarget("3", 5)).toEqual({ type: "page", page: 2 });
    expect(parseTarget("6", 5)).toBeNull();
    expect(parseTarget("0", 5)).toBeNull();
    expect(parseTarget("https://pdfpliego.com/a", 5)).toEqual({ type: "uri", uri: "https://pdfpliego.com/a" });
    expect(parseTarget("mailto:hola@pdfpliego.com", 5)).toEqual({ type: "uri", uri: "mailto:hola@pdfpliego.com" });
    expect(parseTarget("pdfpliego.com", 5)).toEqual({ type: "uri", uri: "https://pdfpliego.com" });
    expect(parseTarget("hola mundo", 5)).toBeNull();
    expect(parseTarget("https://pdfpliego.com/ñ", 5)).toBeNull();
    expect(parseTarget("  ", 5)).toBeNull();
  });
  it("muestra el destino como lo escribiría la persona", () => {
    expect(targetText({ type: "page", page: 2 })).toBe("3");
    expect(targetText({ type: "uri", uri: "https://a.es" })).toBe("https://a.es");
    expect(targetText(null)).toBe("");
  });
  it("coloca el formulario sin que se corte por el borde de la página", () => {
    const size = { w: 320, h: 280 };
    const page = { w: 600, h: 800 };
    // Cabe debajo.
    expect(placeForm({ x: 50, below: 100, above: 90 }, size, page)).toEqual({ left: 50, top: 100 });
    // No cabe debajo (texto al final de la página): va encima.
    expect(placeForm({ x: 50, below: 760, above: 740 }, size, page)).toEqual({ left: 50, top: 460 });
    // No cabe ni debajo ni encima: pegado al borde inferior.
    expect(placeForm({ x: 50, below: 200, above: 190 }, size, { w: 600, h: 300 })).toEqual({ left: 50, top: 20 });
    // Se mantiene dentro por la derecha.
    expect(placeForm({ x: 500, below: 100, above: 90 }, size, page).left).toBe(280);
  });
});
