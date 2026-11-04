import { describe, expect, it } from "vitest";
import { sizeLabel } from "./attachments";

describe("tamaño de adjuntos", () => {
  it("elige la unidad", () => {
    expect(sizeLabel(512)).toBe("512 B");
    expect(sizeLabel(1536)).toBe("1,5 KB");
    expect(sizeLabel(3 * 1024 * 1024)).toBe("3,0 MB");
  });
});
