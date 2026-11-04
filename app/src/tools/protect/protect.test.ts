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
