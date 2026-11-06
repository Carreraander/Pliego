/**
 * Aviso antes de editar un documento firmado. Editar el contenido (texto, imágenes, objetos, enlaces) reescribe el
 * archivo y deja las firmas digitales sin validez; se avisa una vez por archivo y sesión, antes de la primera edición
 * (la copia de seguridad se guarda igualmente). El diálogo lo dibuja `components/SignedEditPrompt.tsx`.
 */
import { invoke } from "@tauri-apps/api/core";
import { createSignal } from "solid-js";
import { withPassword } from "./passwords";

export interface SignedEditRequest {
  path: string;
  resolve: (ok: boolean) => void;
}

/** Petición pendiente; `null` si no hay ninguna. */
export const [signedEditRequest, setSignedEditRequest] = createSignal<SignedEditRequest | null>(null);

/** Archivos ya comprobados o aceptados en esta sesión. */
const settled = new Set<string>();

export function forgetSignedChoice(path: string): void {
  settled.delete(path);
}

/** Cierra el diálogo con la respuesta de la persona. */
export function answerSignedEdit(ok: boolean): void {
  const r = signedEditRequest();
  setSignedEditRequest(null);
  if (r && ok) settled.add(r.path);
  r?.resolve(ok);
}

/**
 * `true` si se puede editar `path`: no está firmado o la persona acepta perder la validez de las firmas.
 * Si no se puede saber (error al abrirlo), deja pasar: la propia edición mostrará el error.
 */
export async function confirmEditSigned(path: string): Promise<boolean> {
  if (settled.has(path)) return true;
  let signed = false;
  try {
    signed = (await invoke<boolean>("edit_is_signed", withPassword({ path }))) === true;
  } catch {
    return true;
  }
  if (!signed) {
    settled.add(path);
    return true;
  }
  return new Promise((resolve) => setSignedEditRequest({ path, resolve }));
}
