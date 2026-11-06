import { describe, expect, it } from "vitest";
import { cycleTab, targetPage, zoomKeyDirection } from "./commands";

describe("cycleTab", () => {
  it("recorre las pestañas con vuelta", () => {
    expect(cycleTab([4, 7, 9], 7, 1)).toBe(9);
    expect(cycleTab([4, 7, 9], 9, 1)).toBe(4);
    expect(cycleTab([4, 7, 9], 4, -1)).toBe(9);
  });
  it("sin pestaña activa va a la primera o a la última", () => {
    expect(cycleTab([4, 7], null, 1)).toBe(4);
    expect(cycleTab([4, 7], null, -1)).toBe(7);
    expect(cycleTab([], null, 1)).toBeNull();
  });
});

describe("targetPage", () => {
  it("no se sale del documento", () => {
    expect(targetPage("first", 5, 10)).toBe(0);
    expect(targetPage("last", 5, 10)).toBe(9);
    expect(targetPage("prev", 0, 10)).toBe(0);
    expect(targetPage("next", 9, 10)).toBe(9);
    expect(targetPage("next", 3, 10)).toBe(4);
    expect(targetPage("last", 0, 0)).toBe(0);
  });
});

describe("zoomKeyDirection", () => {
  it("acepta las variantes de cada teclado", () => {
    expect(zoomKeyDirection("=")).toBe(1);
    expect(zoomKeyDirection("+")).toBe(1);
    expect(zoomKeyDirection("_")).toBe(-1);
    expect(zoomKeyDirection("a")).toBe(0);
  });
});
