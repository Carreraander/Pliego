import { invoke } from "@tauri-apps/api/core";
import { toast } from "../../ui";
import type { OpenDoc } from "../../state/documents";
import { errorText } from "../run";
import { setUndoDepth } from "./state";
import { at } from "./text";
import { withPassword } from "../../state/passwords";

/** Añade un comentario al PDF abierto (el servicio guarda la copia de seguridad la primera vez y recarga el visor). */
export async function addComment(doc: OpenDoc, spec: Record<string, unknown>): Promise<boolean> {
  try {
    setUndoDepth(await invoke<number>("annot_add", withPassword({ docId: doc.id, path: doc.path, spec, suffix: at("annotate.backupSuffix") })));
    return true;
  } catch (e) {
    toast({ tone: "danger", text: errorText(e) });
    return false;
  }
}

export async function undoComment(doc: OpenDoc): Promise<void> {
  try {
    setUndoDepth(await invoke<number>("annot_undo", { docId: doc.id, path: doc.path }));
  } catch (e) {
    toast({ tone: "danger", text: errorText(e) });
  }
}
