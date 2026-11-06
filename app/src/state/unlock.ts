/**
 * Contraseñas de los archivos que una herramienta recibe **además** del documento abierto (Combinar, Comparar con otro
 * PDF, Insertar páginas de otro PDF). Antes de lanzar el trabajo se comprueba cada uno: si está protegido se pide su
 * contraseña (una sola vez por archivo y panel) en vez de dejar que el trabajo falle al final.
 *
 * Las contraseñas no se guardan en ningún sitio: las devuelve `unlockFile` y las conserva el panel mientras está abierto.
 * La del propio documento abierto sale de `state/passwords.ts`.
 */
import { invoke } from "@tauri-apps/api/core";
import { createSignal } from "solid-js";
import { engineText } from "../i18n/engine";
import { passwordFor } from "./passwords";

export type PasswordStatus = "open" | "required" | "wrong";

export interface PasswordRequest {
  path: string;
  /** La contraseña anterior no valía. */
  wrong: boolean;
  resolve: (password: string | null) => void;
}

/** Petición pendiente que dibuja `components/PasswordPrompt.tsx`; `null` si no hay ninguna. */
export const [passwordRequest, setPasswordRequest] = createSignal<PasswordRequest | null>(null);

export const passwordStatus = (path: string, password: string | null): Promise<PasswordStatus> =>
  invoke<PasswordStatus>("password_status", { path, password });

/** Muestra el diálogo y devuelve lo escrito, o `null` si se cancela. */
export function askPassword(path: string, wrong: boolean): Promise<string | null> {
  return new Promise((resolve) => setPasswordRequest({ path, wrong, resolve }));
}

/** Cierra el diálogo con una respuesta. */
export function answerPassword(password: string | null): void {
  const r = passwordRequest();
  setPasswordRequest(null);
  r?.resolve(password);
}

export type Unlocked = { ok: true; password: string | null } | { ok: false; reason: "cancelled" | "error"; message?: string };

/**
 * Deja listo `path` para usarlo en un trabajo. Devuelve su contraseña (`null` si no la necesita) o `ok: false` si la
 * persona cancela. `known` es la que ya se tiene (la recordada del documento abierto, por ejemplo).
 */
export async function unlockFile(path: string, known: string | null = passwordFor(path)): Promise<Unlocked> {
  try {
    let password = known;
    let status = await passwordStatus(path, password);
    while (status !== "open") {
      const typed = await askPassword(path, status === "wrong");
      if (typed === null) return { ok: false, reason: "cancelled" };
      password = typed;
      status = await passwordStatus(path, password);
    }
    return { ok: true, password };
  } catch (e) {
    const message = engineText(e && typeof e === "object" && "message" in e ? String((e as { message: unknown }).message) : String(e));
    return { ok: false, reason: "error", message };
  }
}
