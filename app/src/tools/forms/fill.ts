import { invoke } from "@tauri-apps/api/core";
import { toast } from "../../ui";
import type { OpenDoc } from "../../state/documents";
import { setUndoDepth } from "../annotate/state";
import { at } from "../annotate/text";
import { errorText } from "../run";
import { withPassword } from "../../state/passwords";

/** Rellena un campo en el PDF abierto (copia de seguridad la primera vez; el visor se recarga). */
export async function fillField(doc: OpenDoc, name: string, value: string | boolean): Promise<boolean> {
  try {
    setUndoDepth(await invoke<number>("forms_fill", withPassword({ docId: doc.id, path: doc.path, values: { [name]: value }, suffix: at("annotate.backupSuffix") })));
    return true;
  } catch (e) {
    toast({ tone: "danger", text: errorText(e) });
    return false;
  }
}
