import { invoke } from "@tauri-apps/api/core";
import { engineText } from "../../i18n/engine";
import { toast } from "../../ui";
import type { OpenDoc } from "../../state/documents";
import { setUndoDepth } from "../annotate/state";
import { errorText } from "../run";
import { api } from "../../api";
import type { Box, ImageObj, LinkItem, LinkTarget, ObjectKind, TextBlock, TextStyle, Transform, VectorObj } from "./geometry";
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

/** Listas por página (imágenes, vectores, enlaces) con la misma caché por revisión que los bloques de texto. */
function cached<T>(command: string) {
  const cache = new Map<string, Promise<T[]>>();
  return (doc: OpenDoc, page: number): Promise<T[]> => {
    const key = `${doc.path}:${doc.revision}:${page}`;
    let hit = cache.get(key);
    if (!hit) {
      if (cache.size > 40) cache.clear();
      hit = invoke<T[]>(command, withPassword({ path: doc.path, page })).catch((e) => { cache.delete(key); toast({ tone: "danger", text: errorText(e) }); return []; });
      cache.set(key, hit);
    }
    return hit;
  };
}
export const loadImages = cached<ImageObj>("edit_images");
export const loadVectors = cached<VectorObj>("edit_vectors");
export const loadLinks = cached<LinkItem>("edit_links");

async function run(command: string, args: Record<string, unknown>): Promise<boolean> {
  try {
    // El servicio guarda la copia de seguridad la primera vez y recarga el visor; la revisión nueva invalida la caché.
    const out = await invoke<Outcome>(command, withPassword({ ...args, suffix: et("edit.backupSuffix") }));
    setUndoDepth(out.undo_depth);
    toast({ tone: "ok", text: et("edit.done") });
    for (const w of out.warnings) toast({ tone: "warn", text: et("edit.warning", { text: engineText(w) }) });
    return true;
  } catch (e) {
    toast({ tone: "danger", text: errorText(e) });
    return false;
  }
}

export const replaceText = (doc: OpenDoc, b: TextBlock, text: string, style: TextStyle, flow = true) =>
  run("edit_text_replace", { docId: doc.id, path: doc.path, page: b.page, block: b.id, text, style, flow });
export const insertText = (doc: OpenDoc, page: number, rect: Box, text: string, style: TextStyle) =>
  run("edit_text_insert", { docId: doc.id, path: doc.path, page, rect, text, style });

const base = (doc: OpenDoc, page: number) => ({ docId: doc.id, path: doc.path, page });

/** Pide una imagen al usuario y la inserta ocupando `rect`. */
export async function insertImage(doc: OpenDoc, page: number, rect: Box): Promise<boolean> {
  const image = await api.pickImageFile(et("edit.image.pick"));
  return image ? run("edit_image_insert", { ...base(doc, page), rect, image }) : false;
}
export async function replaceImage(doc: OpenDoc, page: number, id: number): Promise<boolean> {
  const image = await api.pickImageFile(et("edit.image.pick"));
  return image ? run("edit_image_replace", { ...base(doc, page), id, image }) : false;
}
export const cropImage = (doc: OpenDoc, page: number, id: number, rect: Box) => run("edit_image_crop", { ...base(doc, page), id, rect });
export const transformObject = (doc: OpenDoc, page: number, kind: ObjectKind, id: number, transform: Transform) =>
  run("edit_object_transform", { ...base(doc, page), kind, id, transform });
export const deleteObject = (doc: OpenDoc, page: number, kind: ObjectKind, id: number) => run("edit_object_delete", { ...base(doc, page), kind, id });
export const addLink = (doc: OpenDoc, page: number, rect: Box, target: LinkTarget) => run("edit_link_add", { ...base(doc, page), rect, target });
export const updateLink = (doc: OpenDoc, page: number, id: number, target: LinkTarget) => run("edit_link_update", { ...base(doc, page), id, target });
export const deleteLink = (doc: OpenDoc, page: number, id: number) => run("edit_link_delete", { ...base(doc, page), id });
