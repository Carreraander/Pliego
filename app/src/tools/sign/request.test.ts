import { describe, expect, it } from "vitest";
import { emptyKey, keySource, missing, signerSpecs, signOptions, visibleRect, type SignForm } from "./request";

const form = (over: Partial<SignForm> = {}): SignForm => ({
  key: { ...emptyKey(), thumbprint: "AB12" }, level: "b-b", tsa: "", visible: false, page: 1, corner: "bottom-right",
  certify: 0, reason: "", location: "", contact: "", ...over,
});

describe("firma digital: argumentos", () => {
  it("coloca la firma visible en cada esquina con margen", () => {
    const a4 = { width: 595, height: 842 };
    expect(visibleRect(a4, "bottom-left")).toEqual([36, 36, 200, 60]);
    expect(visibleRect(a4, "bottom-right")).toEqual([359, 36, 200, 60]);
    expect(visibleRect(a4, "top-left")).toEqual([36, 746, 200, 60]);
    expect(visibleRect(a4, "top-right")).toEqual([359, 746, 200, 60]);
  });
  it("elige la clave del almacén o del archivo y avisa si falta", () => {
    expect(keySource(emptyKey())).toBeNull();
    expect(keySource({ ...emptyKey(), thumbprint: "AB" })).toEqual({ kind: "store", thumbprint: "AB" });
    expect(keySource({ ...emptyKey(), source: "pfx", pfxPath: "C:\\a.pfx", password: "x" })).toEqual({ kind: "pfx", path: "C:\\a.pfx", password: "x" });
    expect(missing(form({ key: emptyKey() }))).toBe("key");
  });
  it("los niveles con sello de tiempo piden un servidor http", () => {
    expect(missing(form())).toBeNull();
    expect(missing(form({ level: "b-t" }))).toBe("tsa");
    expect(missing(form({ level: "b-lta", tsa: "ftp://x" }))).toBe("tsa");
    expect(missing(form({ level: "b-t", tsa: "http://tsa.example/ts" }))).toBeNull();
    expect(missing(form({ visible: true, page: 0 }))).toBe("page");
  });
  it("genera SignOptions solo con lo indicado (páginas desde 0)", () => {
    expect(signOptions(form())).toEqual({ level: "b-b" });
    const o = signOptions(form({ level: "b-t", tsa: " http://t/ ", visible: true, page: 3, certify: 2, reason: " Aprobado ", location: "" }), { width: 595, height: 842 });
    expect(o).toEqual({ level: "b-t", timestamp_url: "http://t/", visible: { page: 2, rect: [359, 36, 200, 60] }, page: 2, certify: 2, reason: "Aprobado" });
  });
  it("el flujo ignora filas sin nombre y recorta espacios", () => {
    expect(signerSpecs([
      { name: " Ana ", email: "a@x.es", certificateContains: "" },
      { name: " ", email: "b@x.es", certificateContains: "" },
      { name: "Beto", email: "", certificateContains: "BETO" },
    ])).toEqual([{ name: "Ana", email: "a@x.es" }, { name: "Beto", certificate_contains: "BETO" }]);
  });
});
