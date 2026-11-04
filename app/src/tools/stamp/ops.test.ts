import { describe, expect, it } from "vitest";
import { backgroundOp, batesOp, headerFooterOp, watermarkOp } from "./ops";

const st = { font: "Courier", size: "12", color: "#ff0000" };

describe("operaciones de sellado", () => {
  it("marca de agua de texto", () => {
    expect(watermarkOp({ kind: "text", text: "BORRADOR", style: st, image: null, scalePct: 50, opacityPct: 30, rotation: "45", behind: true })).toEqual({
      op: "watermark", content: { kind: "text", text: "BORRADOR", style: { font: "Courier", size: 12, color: [1, 0, 0] } },
      opacity: 0.3, rotation: 45, behind: true,
    });
  });
  it("marca de agua de imagen no gira", () => {
    const o = watermarkOp({ kind: "image", text: "", style: st, image: "C:\\logo.png", scalePct: 40, opacityPct: 100, rotation: "45", behind: false });
    expect(o.content).toEqual({ kind: "image", path: "C:\\logo.png", scale: 0.4 });
    expect(o.rotation).toBe(0);
  });
  it("encabezado y pie", () => {
    const o = headerFooterOp({ header: ["", "Contrato", ""], footer: ["", "{page}", ""], style: st, margin: "40" });
    expect(o.header).toEqual({ left: "", center: "Contrato", right: "" });
    expect(o.margin).toBe(40);
  });
  it("fondo de color e imagen", () => {
    expect(backgroundOp({ kind: "color", color: "#000000", image: null, opacityPct: 50 }).fill).toEqual({ kind: "color", rgb: [0, 0, 0] });
    expect(backgroundOp({ kind: "image", color: "", image: "a.png", opacityPct: 100 }).fill).toEqual({ kind: "image", path: "a.png" });
  });
  it("Bates con valores no válidos usa los de por defecto", () => {
    const o = batesOp({ prefix: "X-", suffix: "", digits: "zz", start: "-3", corner: "top-left", style: st });
    expect([o.digits, o.start, o.corner]).toEqual([6, 1, "top_left"]);
  });
});
