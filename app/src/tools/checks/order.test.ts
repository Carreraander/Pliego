import { describe, expect, it } from "vitest";
import { backwardsPages, orderRows } from "./order";

const item = (page: number | null, role: string, has_alt = false) => ({ page, role, has_alt });

describe("orden de lectura", () => {
  it("numera y marca figuras sin texto alternativo", () => {
    const rows = orderRows([item(0, "H1"), item(0, "Figure"), item(1, "Figure", true)]);
    expect(rows.map((r) => r.n)).toEqual([1, 2, 3]);
    expect(rows.map((r) => r.noAlt)).toEqual([false, true, false]);
  });
  it("detecta páginas donde la lectura retrocede", () => {
    expect(backwardsPages([item(0, "P"), item(2, "P"), item(1, "P"), item(null, "P"), item(1, "P")])).toEqual([2]);
    expect(backwardsPages([item(0, "P"), item(1, "P")])).toEqual([]);
  });
});
