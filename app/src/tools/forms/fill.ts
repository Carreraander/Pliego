import { invoke } from "@tauri-apps/api/core";
import { toast } from "../../ui";
import type { OpenDoc } from "../../state/documents";
import { setUndoDepth } from "../annotate/state";
import { at } from "../annotate/text";
import { errorText } from "../run";
import type { KeystrokeResult } from "./keystroke";

/** Reflejo de `FormEdit` (comandos `forms_fill` y `forms_run_action`). */
interface FormEdit { undo_depth: number; messages: string[] }

/** Avisos de los scripts del formulario (`app.alert`, `console.println`, acciones bloqueadas). */
function showMessages(messages: string[]): void {
  for (const text of messages) toast({ tone: "warn", text });
}

/** Rellena un campo en el PDF abierto, ejecutando el JavaScript del formulario (validación, cálculo y formato). Copia de seguridad la primera vez; el visor se recarga. */
export async function fillField(doc: OpenDoc, name: string, value: string | boolean): Promise<boolean> {
  try {
    const r = await invoke<FormEdit>("forms_fill", { docId: doc.id, path: doc.path, values: { [name]: value }, suffix: at("annotate.backupSuffix") });
    setUndoDepth(r.undo_depth);
    showMessages(r.messages);
    return true;
  } catch (e) {
    toast({ tone: "danger", text: errorText(e) });
    return false;
  }
}

/** Ejecuta la acción de un botón (o de otro campo) del formulario. */
export async function runFieldAction(doc: OpenDoc, name: string, trigger = "U"): Promise<boolean> {
  try {
    const r = await invoke<FormEdit>("forms_run_action", { docId: doc.id, path: doc.path, name, trigger, suffix: at("annotate.backupSuffix") });
    setUndoDepth(r.undo_depth);
    showMessages(r.messages);
    return true;
  } catch (e) {
    toast({ tone: "danger", text: errorText(e) });
    return false;
  }
}

/** Pasa una pulsación en curso por el script de tecleo del campo. Si algo falla se acepta tal cual (el valor se valida de nuevo al confirmar). */
export async function filterKeystroke(doc: OpenDoc, name: string, change: string, current: string, start: number, end: number): Promise<KeystrokeResult> {
  try {
    const r = await invoke<KeystrokeResult>("forms_keystroke", { path: doc.path, name, change, current, selStart: start, selEnd: end });
    showMessages(r.messages);
    return r;
  } catch {
    return { accepted: true, change, messages: [] };
  }
}
