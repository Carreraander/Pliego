import { describe, expect, it } from "vitest";
import { overall, verdict, verdictTone, type SignatureReport } from "./status";

const report = (over: Partial<SignatureReport> = {}): SignatureReport => ({
  field: "Firma1", signer: "Ana", subject: "CN=Ana", signing_time: null, reason: null, location: null, integrity: true,
  covers_whole_document: true, modifications_allowed: true, has_dss: false, certification: null, level: "b-b", chain: "trusted",
  has_timestamp: false, document_timestamp: null, problems: [], ...over,
});

describe("estado de las firmas", () => {
  it("distingue válida, emisor desconocido, modificada e inválida", () => {
    expect(verdict(report())).toBe("valid");
    expect(verdict(report({ chain: "untrusted-root" }))).toBe("unknown-issuer");
    expect(verdict(report({ chain: "incomplete" }))).toBe("unknown-issuer");
    expect(verdict(report({ modifications_allowed: false }))).toBe("modified");
    expect(verdict(report({ integrity: false }))).toBe("invalid");
    expect(verdict(report({ chain: "invalid" }))).toBe("invalid");
    expect(verdict(report({ document_timestamp: { valid: false } }))).toBe("unknown-issuer");
  });
  it("el peor veredicto manda en el resumen", () => {
    expect(overall([])).toBeNull();
    expect(overall([report(), report({ chain: "untrusted-root" })])).toBe("unknown-issuer");
    expect(overall([report({ chain: "untrusted-root" }), report({ integrity: false })])).toBe("invalid");
    expect(verdictTone("valid")).toBe("ok");
    expect(verdictTone("unknown-issuer")).toBe("warn");
    expect(verdictTone("modified")).toBe("danger");
  });
});
