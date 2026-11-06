import { forgetPassword, rememberPassword } from "./passwords";
import { createSignal } from "solid-js";
import { api, events, isApiError, type DocId, type PageSize } from "../api";
import { engineText } from "../i18n/engine";
import { fitWidthZoom, type Rotation } from "../viewer/layout";

export interface OpenDoc {
  id: DocId;
  path: string;
  title: string;
  sizes: PageSize[];
  revision: number;
}

export const [docs, setDocs] = createSignal<OpenDoc[]>([]);
export const [activeId, setActiveId] = createSignal<DocId | null>(null);
export const [zoom, setZoom] = createSignal(1);
export const [rotation, setRotation] = createSignal<Rotation>(0);
export const [currentPageIndex, setCurrentPageIndex] = createSignal(0);

export const activeDoc = () => docs().find((d) => d.id === activeId()) ?? null;

function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

/** Abre un archivo. Devuelve "password" si hace falta contraseña (o es incorrecta), "ok" o el mensaje de error. */
export async function openPath(path: string, viewportWidth: number, password?: string): Promise<"ok" | "password" | string> {
  const known = docs().find((d) => d.path === path);
  if (known && password === undefined) {
    setActiveId(known.id);
    return "ok";
  }
  try {
    const id = await api.openDocument(path, password);
    if (password) rememberPassword(path, password); // para los trabajos que se lancen luego sobre este archivo
    const [info, sizes] = await Promise.all([api.documentInfo(id), api.pageSizes(id)]);
    const doc: OpenDoc = { id, path, title: info.title || fileName(path), sizes, revision: 0 };
    setDocs((d) => [...d, doc]);
    setActiveId(id);
    setRotation(0);
    setCurrentPageIndex(0);
    if (sizes[0]) setZoom(Math.min(1.5, fitWidthZoom(sizes[0], 0, viewportWidth)));
    return "ok";
  } catch (e) {
    if (isApiError(e)) return e.code === "password" ? "password" : engineText(e.message);
    return engineText(String(e));
  }
}

export async function closeDoc(id: DocId): Promise<void> {
  const path = docs().find((d) => d.id === id)?.path;
  if (path) forgetPassword(path);
  await api.closeDocument(id).catch(() => undefined);
  const rest = docs().filter((d) => d.id !== id);
  setDocs(rest);
  if (activeId() === id) setActiveId(rest.at(-1)?.id ?? null);
}

/** Escucha `document-changed` para invalidar teselas (la URL lleva la revisión). */
export function listenForChanges(): Promise<() => void> {
  return events.onDocumentChanged(({ doc_id, revision }) =>
    setDocs((all) => all.map((d) => (d.id === doc_id ? { ...d, revision } : d))),
  );
}
