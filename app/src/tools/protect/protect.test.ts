import { beforeEach, describe, expect, it } from "vitest";
import { setLocale } from "../../i18n";
import { modes } from "../../shell/slots";
import { registerBaseTools } from "../base";
import { getTool, NO_DOCUMENT, setDocFacts, toolState } from "../registry";
import { registerProtectTools } from "./index";
import { ALL_ALLOWED, buildProtectRequest, buildSanitizeRequest, SANITIZE_ALL, type ProtectForm } from "./request";
import { pt } from "./strings";

const form = (over: Partial<ProtectForm> = {}): ProtectForm => ({
  userPassword: "abre", repeat: "abre", ownerPassword: "", current: "", cipher: "Aes256", permissions: { ...ALL_ALLOWED }, ...over,
});

beforeEach(() => { setLocale("es"); setDocFacts(NO_DOCUMENT); });

describe("registro de Proteger", () => {
  it("sustituye a las fantasma y registra el modo con el mismo id", () => {
    registerBaseTools();
    expect(getTool("protect.protect")?.ghost).toBe(true);
    const off = registerProtectTools();
    expect(getTool("protect.protect")?.ghost).toBeUndefined();
    expect(getTool("protect.sanitize")?.ghost).toBeUndefined();
    expect(modes().some((m) => m.id === "protect.protect")).toBe(true);
    expect(toolState(getTool("protect.sanitize")!).enabled).toBe(false);
    setDocFacts({ document: true });
    expect(toolState(getTool("protect.sanitize")!).enabled).toBe(true);
    off();
  });
});

describe("petición de cifrado", () => {
  it("acepta solo contraseña de apertura", () => {
    const r = buildProtectRequest("C:\\a\\x.pdf", form());
    expect(r).toMatchObject({ ok: true, request: { path: "C:\\a\\x.pdf", userPassword: "abre", password: null, cipher: "Aes256" } });
  });
  it("exige alguna contraseña y que coincidan", () => {
    expect(buildProtectRequest("x", form({ userPassword: "", repeat: "" }))).toEqual({ ok: false, error: "protect.err.none" });
    expect(buildProtectRequest("x", form({ repeat: "otra" }))).toEqual({ ok: false, error: "protect.err.mismatch" });
  });
  it("restringir permisos pide contraseña de permisos", () => {
    const perms = { ...ALL_ALLOWED, copy: false };
    expect(buildProtectRequest("x", form({ permissions: perms }))).toEqual({ ok: false, error: "protect.err.owner" });
    expect(buildProtectRequest("x", form({ permissions: perms, ownerPassword: "manda" })).ok).toBe(true);
  });
  it("limita las contraseñas a 127 bytes", () => {
    const long = "ñ".repeat(64);
    expect(buildProtectRequest("x", form({ userPassword: long, repeat: long }))).toEqual({ ok: false, error: "protect.err.long" });
  });
  it("pasa la contraseña actual si el documento ya estaba protegido", () => {
    const r = buildProtectRequest("x", form({ current: "vieja" }));
    expect(r).toMatchObject({ ok: true, request: { password: "vieja" } });
  });
  it("todos los errores tienen texto en español e inglés", () => {
    for (const key of ["protect.err.none", "protect.err.mismatch", "protect.err.owner", "protect.err.long", "sanitize.err.none"]) {
      setLocale("es"); const es = pt(key);
      setLocale("en"); const en = pt(key);
      expect(es).not.toBe(key); expect(en).not.toBe(key); expect(es).not.toBe(en);
    }
  });
});

describe("petición de saneado", () => {
  it("por defecto quita todo", () => {
    expect(buildSanitizeRequest("x", "", { ...SANITIZE_ALL })).toMatchObject({ ok: true, request: { password: null, options: SANITIZE_ALL } });
  });
  it("sin ninguna opción marcada falla", () => {
    const none = Object.fromEntries(Object.keys(SANITIZE_ALL).map((k) => [k, false])) as typeof SANITIZE_ALL;
    expect(buildSanitizeRequest("x", "", none)).toEqual({ ok: false, error: "sanitize.err.none" });
  });
});

import { buildRedactRequest, patternNames, type RedactForm } from "./request";
import { addArea, areasOf, clearAreas, resetRedact, toArea } from "./redactState";

const rform = (over: Partial<RedactForm> = {}): RedactForm => ({
  patterns: [], regex: "", useRegex: false, areas: [], fill: "Black", images: "Pixels", removeLineArt: true, current: "", ...over,
});

describe("petición de redacción", () => {
  it("pide zonas o patrones", () => {
    expect(buildRedactRequest("x", rform())).toEqual({ ok: false, error: "redact.err.nothing" });
    expect(buildRedactRequest("x", rform({ patterns: ["dni", "iban"] }))).toMatchObject({ ok: true, request: { patterns: ["dni", "iban"], areas: [] } });
    expect(buildRedactRequest("x", rform({ areas: [{ page: 0, x0: 1, y0: 1, x1: 9, y1: 9 }] })).ok).toBe(true);
  });
  it("añade la expresión propia como regex: y exige que no esté vacía", () => {
    expect(patternNames({ patterns: ["dni"], useRegex: true, regex: " \\d{4} " })).toEqual(["dni", "regex:\\d{4}"]);
    expect(buildRedactRequest("x", rform({ useRegex: true }))).toEqual({ ok: false, error: "redact.err.regex" });
  });
  it("traduce las opciones al formato del motor", () => {
    const r = buildRedactRequest("x", rform({ patterns: ["email"], fill: "None", images: "Remove", removeLineArt: false, current: "pw" }));
    expect(r).toMatchObject({ ok: true, request: { password: "pw", options: { fill: "None", images: "Remove", remove_line_art: false } } });
  });
  it("los errores de redacción tienen texto en los dos idiomas", () => {
    for (const key of ["redact.err.nothing", "redact.err.regex"]) {
      setLocale("es"); const es = pt(key); setLocale("en"); const en = pt(key);
      expect(es).not.toBe(key); expect(en).not.toBe(es);
    }
  });
});

describe("zonas marcadas a mano", () => {
  const size = { width: 100, height: 200 };
  it("normaliza el arrastre en cualquier dirección y lo recorta a la página", () => {
    expect(toArea(2, { x: 50, y: 60 }, { x: 10, y: 20 }, size)).toEqual({ page: 2, x0: 10, y0: 20, x1: 50, y1: 60 });
    expect(toArea(0, { x: -5, y: 10 }, { x: 150, y: 300 }, size)).toEqual({ page: 0, x0: 0, y0: 10, x1: 100, y1: 200 });
  });
  it("ignora un clic o un arrastre minúsculo", () => {
    expect(toArea(0, { x: 10, y: 10 }, { x: 11, y: 11 }, size)).toBeNull();
  });
  it("guarda las zonas por documento", () => {
    resetRedact();
    addArea(1, { page: 0, x0: 0, y0: 0, x1: 10, y1: 10 });
    addArea(2, { page: 1, x0: 0, y0: 0, x1: 10, y1: 10 });
    expect(areasOf(1)).toHaveLength(1);
    clearAreas(1);
    expect(areasOf(1)).toHaveLength(0);
    expect(areasOf(2)).toHaveLength(1);
    resetRedact();
  });
});
