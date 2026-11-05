import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import { forgetPassword, rememberPassword } from "./passwords";
import { answerPassword, passwordRequest, unlockFile } from "./unlock";

/** Servicio simulado: `locked[path]` es la contraseña de ese archivo; lo demás se abre sin ella. */
function backend(locked: Record<string, string>) {
  invoke.mockImplementation(async (cmd: string, args: { path: string; password: string | null }) => {
    expect(cmd).toBe("password_status");
    const want = locked[args.path];
    if (want === undefined || args.password === want) return "open";
    return args.password ? "wrong" : "required";
  });
}
const tick = () => new Promise((r) => setTimeout(r, 0));

describe("unlockFile", () => {
  beforeEach(() => { invoke.mockReset(); forgetPassword("C:\\a.pdf"); });

  it("un archivo sin protección se usa tal cual y no pregunta nada", async () => {
    backend({});
    expect(await unlockFile("C:\\a.pdf")).toEqual({ ok: true, password: null });
    expect(passwordRequest()).toBeNull();
  });

  it("usa la contraseña recordada del documento abierto sin preguntar", async () => {
    backend({ "C:\\a.pdf": "abre" });
    rememberPassword("C:\\a.pdf", "abre");
    expect(await unlockFile("C:\\a.pdf")).toEqual({ ok: true, password: "abre" });
    expect(passwordRequest()).toBeNull();
  });

  it("pide la contraseña de un archivo protegido y vuelve a preguntar si es incorrecta", async () => {
    backend({ "C:\\b.pdf": "clave" });
    const result = unlockFile("C:\\b.pdf");
    await tick();
    expect(passwordRequest()).toMatchObject({ path: "C:\\b.pdf", wrong: false });
    answerPassword("mala");
    await tick();
    expect(passwordRequest()).toMatchObject({ path: "C:\\b.pdf", wrong: true });
    answerPassword("clave");
    expect(await result).toEqual({ ok: true, password: "clave" });
    expect(passwordRequest()).toBeNull();
  });

  it("cancelar el diálogo cancela el archivo", async () => {
    backend({ "C:\\b.pdf": "clave" });
    const result = unlockFile("C:\\b.pdf");
    await tick();
    answerPassword(null);
    expect(await result).toEqual({ ok: false, reason: "cancelled" });
  });

  it("si la contraseña recordada ya no vale, la pregunta como incorrecta", async () => {
    backend({ "C:\\a.pdf": "nueva" });
    rememberPassword("C:\\a.pdf", "vieja");
    const result = unlockFile("C:\\a.pdf");
    await tick();
    expect(passwordRequest()).toMatchObject({ wrong: true });
    answerPassword("nueva");
    expect(await result).toEqual({ ok: true, password: "nueva" });
  });

  it("un fallo del servicio (archivo ilegible) se devuelve como error, sin diálogo", async () => {
    invoke.mockRejectedValue({ code: "io", message: "no se puede leer" });
    expect(await unlockFile("C:\\x.pdf")).toEqual({ ok: false, reason: "error", message: "no se puede leer" });
    expect(passwordRequest()).toBeNull();
  });
});
