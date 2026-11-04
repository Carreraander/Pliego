import { describe, expect, it } from "vitest";
import { formatSize, pageSizeLabel, propertyRows } from "./properties";

const info = { page_count: 3, title: " Contrato ", author: null, subject: "", keywords: null, creator: "Word", producer: null, format: "PDF 1.7", encryption: null };

describe("propiedades del documento", () => {
  it("formatea tamaños y páginas", () => {
    expect(formatSize(900)).toBe("900 B");
    expect(formatSize(1536)).toBe("1,5 KB");
    expect(formatSize(5 * 1024 * 1024)).toBe("5,0 MB");
    expect(pageSizeLabel(595.28, 841.89)).toBe("210 × 297 mm");
  });
  it("omite los campos vacíos y mantiene el orden", () => {
    const rows = propertyRows(info, { size: 2048, modified_secs: 1_700_000_000 }, "C:\\a.pdf", { width: 595, height: 842 }, () => "fecha");
    expect(rows.map((r) => r.key)).toEqual(["title", "creator", "format", "pages", "pageSize", "fileSize", "modified", "path"]);
    expect(rows[0]!.value).toBe("Contrato");
  });
  it("sin datos del archivo ni de página", () => {
    const keys = propertyRows(info, null, "a.pdf", null, () => "").map((r) => r.key);
    expect(keys).not.toContain("fileSize");
    expect(keys).not.toContain("pageSize");
  });
});
