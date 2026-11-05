import { invoke } from "@tauri-apps/api/core";
import { toast } from "../../ui";
import type { OpenDoc } from "../../state/documents";
import { setUndoDepth } from "../annotate/state";
import { errorText } from "../run";
import type { Box, TextBlock, TextStyle } from "./geometry";
import { et } from "./text";
import { withPassword } from "../../state/passwords";

interface Outcome { undo_depth: number; warnings: string[] }

/** Bloques de texto de una página (los comparten las capas que lo pidan en la misma revisión). */
const cache = new Map<string, Promise<TextBlock[]>>();
export function loadBlocks(doc: OpenDoc, page: number): Promise<TextBlock[]> {
  const key = `${doc.path}:${doc.revision}:${page}`;
  let hit = cache.get(key);
  if (!hit) {
    if (cache.size > 40) cache.clear();
    hit = invoke<TextBlock[]>("edit_text_blocks", withPassword({ path: doc.path, page })).catch((e) => { cache.delete(key); toast({ tone: "danger", text: errorText(e) }); return []; });
    cache.set(key, hit);
  }
  return hit;
}

async function run(command: string, args: Record<string, unknown>): Promise<boolean> {
  try {
    // El servicio guarda la copia de seguridad la primera vez y recarga el visor; la revisión nueva invalida la caché.
    const out = await invoke<Outcome>(command, withPassword({ ...args, suffix: et("edit.backupSuffix") }));
    setUndoDepth(out.undo_depth);
    toast({ tone: "ok", text: et("edit.done") });
    for (const w of out.warnings) toast({ tone: "warn", text: et("edit.warning", { text: w }) });
    return true;
  } catch (e) {
    toast({ tone: "danger", text: errorText(e) });
    return false;
  }
}

export const replaceText = (doc: OpenDoc, b: TextBlock, text: string, style: TextStyle) =>
  run("edit_text_replace", { docId: doc.id, path: doc.path, page: b.page, block: b.id, text, style });
export const insertText = (doc: OpenDoc, page: number, rect: Box, text: string, style: TextStyle) =>
  run("edit_text_insert", { docId: doc.id, path: doc.path, page, rect, text, style });
