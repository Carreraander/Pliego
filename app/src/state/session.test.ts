import { describe, expect, it } from "vitest";
import { sessionDocs } from "./session";

describe("sessionDocs", () => {
  it("guarda el orden de las pestañas y la última página de cada una", () => {
    const pages = new Map([["C:\\b.pdf", 4]]);
    expect(sessionDocs(["C:\\a.pdf", "C:\\b.pdf"], pages)).toEqual([{ path: "C:\\a.pdf", page: 0 }, { path: "C:\\b.pdf", page: 4 }]);
    expect(sessionDocs([], pages)).toEqual([]);
  });
});
