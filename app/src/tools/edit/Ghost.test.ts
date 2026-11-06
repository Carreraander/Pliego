import { describe, expect, it } from "vitest";
import { ghostTransform } from "./Ghost";

describe("ghostTransform", () => {
  it("lleva la caja original a llenar la nueva", () => {
    expect(ghostTransform([10, 20, 110, 70], [30, 40, 230, 140], 2)).toBe("scale(2, 2) translate(-20px, -40px)");
  });
  it("al mover sin cambiar el tamaño no escala", () => {
    expect(ghostTransform([10, 20, 110, 70], [50, 60, 150, 110], 1)).toBe("scale(1, 1) translate(-10px, -20px)");
  });
  it("no divide entre cero con una caja sin alto (una línea)", () => {
    const t = ghostTransform([0, 5, 100, 5], [0, 5, 100, 6], 1);
    expect(t).toContain("scale(1, 2)");
  });
});
