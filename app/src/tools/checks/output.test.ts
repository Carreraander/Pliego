import { describe, expect, it } from "vitest";
import { overLimit, percent, plateSrc, spotInks } from "./output";

describe("vista previa de salida", () => {
  it("redondea porcentajes", () => {
    expect(percent(0.504)).toBe(50);
    expect(percent(1.6)).toBe(160);
  });
  it("avisa solo por encima del 300 %", () => {
    expect(overLimit(3)).toBe(false);
    expect(overLimit(3.2)).toBe(true);
    expect(overLimit(2.8, 2.5)).toBe(true);
  });
  it("arma la imagen y separa las tintas directas", () => {
    expect(plateSrc({ channel: "cyan", coverage: 0, png: "AAA" })).toBe("data:image/png;base64,AAA");
    const inks = [{ name: "Cyan", kind: "process" as const, pages: [0] }, { name: "PANTONE 185", kind: "spot" as const, pages: [0] }];
    expect(spotInks(inks).map((i) => i.name)).toEqual(["PANTONE 185"]);
  });
});
