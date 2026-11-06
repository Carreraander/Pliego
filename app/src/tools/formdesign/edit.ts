import { invoke } from "@tauri-apps/api/core";
import { toast } from "../../ui";
import type { OpenDoc } from "../../state/documents";
import { setUndoDepth } from "../annotate/state";
import { errorText } from "../run";
import type { FieldInfo } from "./geometry";
import { setFieldList } from "./state";
import { ft } from "./text";
import { withPassword } from "../../state/passwords";

let cache: { key: string; list: Promise<FieldInfo[]> } | null = null;

/** Campos del documento en la revisión dada; varias páginas comparten la misma petición. */
export function loadFields(doc: OpenDoc): Promise<FieldInfo[]> {
  const key = `${doc.path}:${doc.revision}`;
  if (cache?.key !== key) {
    const list = invoke<FieldInfo[]>("form_fields", withPassword({ path: doc.path })).catch((e) => { toast({ tone: "danger", text: errorText(e) }); return []; });
    cache = { key, list };
    void list.then((l) => { if (cache?.key === key) setFieldList(l); });
  }
  return cache.list;
}

async function run(command: string, args: Record<string, unknown>): Promise<boolean> {
  try {
    // El servicio guarda la copia de seguridad la primera vez, recarga el visor y devuelve cuántos cambios se pueden deshacer.
    setUndoDepth(await invoke<number>(command, withPassword({ ...args, suffix: ft("fdesign.backupSuffix") })));
    cache = null;
    return true;
  } catch (e) {
    toast({ tone: "danger", text: errorText(e) });
    return false;
  }
}

export const addField = (doc: OpenDoc, spec: Record<string, unknown>) => run("form_add_field", { docId: doc.id, path: doc.path, spec });
export const updateField = (doc: OpenDoc, name: string, patch: Record<string, unknown>) => run("form_update_field", { docId: doc.id, path: doc.path, name, patch });
export const deleteField = (doc: OpenDoc, name: string) => run("form_delete_field", { docId: doc.id, path: doc.path, name });
