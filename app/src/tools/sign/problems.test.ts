import { describe, expect, it } from "vitest";
import { toolText } from "../strings";
import "./strings";
import { problemLines, type SignatureReport } from "./status";

const report = (over: Partial<SignatureReport>): SignatureReport => ({
  field: null, signer: "Ana", subject: "CN=Ana", signing_time: null, reason: null, location: null, integrity: true,
  covers_whole_document: true, modifications_allowed: true, has_dss: false, certification: null, level: "b-b", chain: "trusted",
  has_timestamp: false, document_timestamp: null, problems: [], problem_keys: [], ...over,
});
const tr = (locale: "es" | "en") => (key: string) => toolText(key, locale);

describe("avisos de «Validar firmas»", () => {
  it("salen en el idioma de la app y no el texto fijo en español del motor", () => {
    const r = report({ problems: ["el sello de tiempo es ilegible"], problem_keys: ["timestamp-unreadable"] });
    expect(problemLines(r, tr("en"))).toEqual(["The timestamp cannot be read"]);
    expect(problemLines(r, tr("es"))).toEqual(["El sello de tiempo es ilegible"]);
  });
  it("los de la cadena ya los muestra la línea de la cadena: no se repiten", () => {
    const r = report({ chain: "incomplete", problems: ["la cadena de certificados está incompleta"], problem_keys: ["chain-incomplete"] });
    expect(problemLines(r, tr("en"))).toEqual([]);
  });
  it("un aviso sin clave conocida se muestra tal cual", () => {
    expect(problemLines(report({ problems: ["otro"], problem_keys: ["other"] }), tr("en"))).toEqual(["otro"]);
    expect(problemLines(report({ problems: ["antiguo"], problem_keys: [] }), tr("en"))).toEqual(["antiguo"]);
  });
  it("todas las claves del motor tienen texto en español e inglés", () => {
    for (const k of ["document-timestamp-invalid", "changes-not-allowed", "timestamp-mismatch", "timestamp-invalid", "timestamp-unreadable"]) {
      expect(toolText(`problem.${k}`, "es")).not.toBe(`problem.${k}`);
      expect(toolText(`problem.${k}`, "en")).not.toBe(toolText(`problem.${k}`, "es"));
    }
    for (const k of ["sign.suffix", "flow.prepare.suffix"]) expect(toolText(k, "en")).not.toBe(toolText(k, "es"));
  });
});
