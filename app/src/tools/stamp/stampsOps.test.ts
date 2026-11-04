import { describe, expect, it } from "vitest";
import { stampPages, stampRequest } from "./stampsOps";

const base = { preset: "aprobado" as const, text: "VISTO", color: "#0000ff", image: null, place: "center" as const };

describe("sellos", () => {
  it("construye las tres fuentes", () => {
    expect(stampRequest({ ...base, kind: "preset", place: "top_right" })).toEqual({ source: { kind: "preset", preset: "aprobado" }, place: "top_right" });
    expect(stampRequest({ ...base, kind: "text" }).source).toEqual({ kind: "text", text: "VISTO", color: [0, 0, 1] });
    expect(stampRequest({ ...base, kind: "image", image: "a.png" }).source).toEqual({ kind: "image", path: "a.png" });
  });
  it("página actual o todas", () => {
    expect(stampPages("current", 0)).toBe("1");
    expect(stampPages("current", 4)).toBe("5");
    expect(stampPages("all", 4)).toBe("all");
  });
});
