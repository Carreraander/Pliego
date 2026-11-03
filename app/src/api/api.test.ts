import { describe, expect, it } from "vitest";
import { tileUrl } from "./index";

describe("tileUrl", () => {
  it("usa http://pliego.localhost en Windows", () => {
    expect(tileUrl(2, 0, 1.5, 1, 3, 4, true)).toBe("http://pliego.localhost/tile/2/0/1.5/1/3?rev=4");
  });
  it("usa pliego://localhost en otros sistemas", () => {
    expect(tileUrl(2, 0, 1, 0, 0, 0, false)).toBe("pliego://localhost/tile/2/0/1/0/0?rev=0");
  });
});
