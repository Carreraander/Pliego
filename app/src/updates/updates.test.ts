import { describe, expect, it } from "vitest";
import { setLocale } from "../i18n";
import type { UpdateSnapshot } from "../settings/updates";
import { downloadPercent, mandatoryNotice, pillKind } from "./logic";
import { ut2 } from "./strings";
import { registerUnsavedProvider, unsavedDocs } from "./unsaved";

const base = {
  current_version: "0.1.0", channel: "stable", mode: "automatic", managed: false, install_kind: "per-user", last_check: null, server: null,
  history: [], whats_new: null, whats_new_notes: null, rollback: null, busy: false,
} as const;
const snap = (p: object): UpdateSnapshot => ({ ...base, ...p }) as unknown as UpdateSnapshot;
const ready = (mandatory = false) => snap({ phase: "ready", version: "0.1.1", mandatory, notes: null, needs_elevation: false });

describe("pastilla y avisos de actualización", () => {
  it("la pastilla solo aparece con una versión lista o una disponible que se puede descargar", () => {
    expect(pillKind(null)).toBeNull();
    expect(pillKind(snap({ phase: "up-to-date" }))).toBeNull();
    expect(pillKind(snap({ phase: "downloading", done: 1, total: 2 }))).toBeNull();
    expect(pillKind(ready())).toBe("ready");
    expect(pillKind(snap({ phase: "available", version: "0.1.1", mandatory: false, notes: null, notes_url: null, can_download: true }))).toBe("available");
    expect(pillKind(snap({ phase: "available", version: "0.1.1", mandatory: false, notes: null, notes_url: null, can_download: false }))).toBeNull();
  });
  it("las obligatorias muestran la barra de aviso, con o sin descarga hecha", () => {
    expect(mandatoryNotice(ready())).toBeNull();
    expect(mandatoryNotice(ready(true))).toEqual({ version: "0.1.1", ready: true });
    expect(mandatoryNotice(snap({ phase: "available", version: "0.2.0", mandatory: true, notes: null, notes_url: null, can_download: true }))).toEqual({ version: "0.2.0", ready: false });
  });
  it("el progreso de descarga se redondea y no pasa de 100", () => {
    expect(downloadPercent(snap({ phase: "downloading", done: 1, total: 3 }))).toBe(33);
    expect(downloadPercent(snap({ phase: "downloading", done: 9, total: 3 }))).toBe(100);
    expect(downloadPercent(snap({ phase: "downloading", done: 0, total: 0 }))).toBeNull();
    expect(downloadPercent(ready())).toBeNull();
  });
  it("los textos existen en español e inglés y sustituyen las variables", () => {
    setLocale("es");
    expect(ut2("mandatory", { version: "0.5.1" })).toContain("0.5.1");
    expect(ut2("rollback", { failed: "0.5.0", restored: "0.4.0" })).toMatch(/0\.4\.0.*0\.5\.0/);
    setLocale("en");
    expect(ut2("pill.ready")).toBe("Update ready");
    setLocale("es");
  });
  it("los documentos sin guardar vienen de quien los registre", () => {
    expect(unsavedDocs()).toEqual([]);
    const off = registerUnsavedProvider(() => [{ title: "a.pdf", save: async () => true }]);
    expect(unsavedDocs().map((d) => d.title)).toEqual(["a.pdf"]);
    off();
    expect(unsavedDocs()).toEqual([]);
  });
});
