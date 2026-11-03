import { describe, expect, it } from "vitest";
import { clamp, layoutMode, INSPECTOR, SIDEPANEL } from "./layout";

describe("layoutMode", () => {
  it("sigue los puntos de corte de 1100 y 900 px", () => {
    expect(layoutMode(1280)).toBe("wide");
    expect(layoutMode(1100)).toBe("wide");
    expect(layoutMode(1099)).toBe("medium");
    expect(layoutMode(900)).toBe("medium");
    expect(layoutMode(899)).toBe("narrow");
    expect(layoutMode(720)).toBe("narrow");
  });
});

describe("clamp", () => {
  it("limita y redondea", () => {
    expect(clamp(100, SIDEPANEL.min, SIDEPANEL.max)).toBe(200);
    expect(clamp(900, SIDEPANEL.min, SIDEPANEL.max)).toBe(480);
    expect(clamp(300.6, INSPECTOR.min, INSPECTOR.max)).toBe(301);
  });
});

import { nextRegion } from "./shortcuts";
describe("nextRegion (F6)", () => {
  const all = ["titlebar", "modebar", "rail", "sidepanel", "canvas", "inspector", "statusbar"];
  it("recorre en el orden del diseño y da la vuelta", () => {
    expect(nextRegion(null, all, 1)).toBe("titlebar");
    expect(nextRegion("canvas", all, 1)).toBe("inspector");
    expect(nextRegion("statusbar", all, 1)).toBe("titlebar");
    expect(nextRegion("titlebar", all, -1)).toBe("statusbar");
  });
  it("salta regiones ausentes", () => {
    expect(nextRegion("rail", ["titlebar", "rail", "canvas"], 1)).toBe("canvas");
  });
});
