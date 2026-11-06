//! Envoltorios tipados de los comandos y eventos de Tauri (ver docs/app-integration.md).
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { t } from "../i18n";

export type DocId = number;

export interface ApiError {
  code: "password" | "io" | "pdf" | "invalid_input" | "unsupported" | "cancelled" | "other";
  message: string;
}
export interface PageSize {
  width: number;
  height: number;
}
export interface DocInfo {
  page_count: number;
  title: string | null;
  author: string | null;
  subject?: string | null;
  keywords?: string | null;
  creator?: string | null;
  producer?: string | null;
  format?: string | null;
  encryption?: string | null;
  is_pdf: boolean;
}
export interface OutlineItem {
  title: string;
  page: number | null;
  uri: string | null;
  children: OutlineItem[];
}
export interface SearchHit {
  page: number;
  /** Cuadriláteros [ulx, uly, urx, ury, llx, lly, lrx, lry] en puntos PDF. */
  quads: number[][];
}
/** Palabra de una página con su caja `[x0, y0, x1, y1]` en puntos de página (origen arriba a la izquierda), en orden de lectura. */
export interface PageWord {
  text: string;
  rect: [number, number, number, number];
  block: number;
  line: number;
}
/** Adónde lleva un enlace de una página: otra página (con la altura pedida, en puntos desde su borde superior) o una dirección. */
export type PageLinkTarget = { type: "page"; page: number; y: number | null } | { type: "uri"; uri: string };
/** Enlace de una página; `rect` = `[x0, y0, x1, y1]` en puntos de página (origen arriba a la izquierda). */
export interface PageLink {
  rect: [number, number, number, number];
  target: PageLinkTarget;
}
export interface DocumentChanged {
  doc_id: DocId;
  revision: number;
}

export const api = {
  openDocument: (path: string, password?: string) => invoke<DocId>("open_document", { path, password }),
  closeDocument: (docId: DocId) => invoke<void>("close_document", { docId }),
  documentInfo: (docId: DocId) => invoke<DocInfo>("document_info", { docId }),
  pageSizes: (docId: DocId) => invoke<PageSize[]>("page_sizes", { docId }),
  outline: (docId: DocId) => invoke<OutlineItem[]>("outline", { docId }),
  pageText: (docId: DocId, page: number) => invoke<string>("page_text", { docId, page }),
  /** Palabras con caja de una página (capa de texto del visor). */
  pageWords: (docId: DocId, page: number) => invoke<PageWord[]>("page_words", { docId, page }),
  /** Enlaces de una página a otra página o a una dirección. */
  pageLinks: (docId: DocId, page: number) => invoke<PageLink[]>("page_links", { docId, page }),
  /** Abre una dirección web o de correo en el sistema. Pide confirmación antes (véase `viewer/links/openUrl.ts`). */
  openWebLink: (url: string) => invoke<void>("open_web_link", { url }),
  search: (docId: DocId, query: string, maxResults?: number) =>
    invoke<SearchHit[]>("search", { docId, query, maxResults }),
  startupFiles: () => invoke<string[]>("startup_files"),
  /** Selector nativo de PDF; devuelve las rutas elegidas (vacío si se cancela). */
  pickPdfFiles: (title?: string) => invoke<string[]>("pick_pdf_files", { title }),
  /** Selector nativo de archivos de cualquier tipo (cartera); vacío si se cancela. */
  pickAnyFiles: (title?: string) => invoke<string[]>("pick_any_files", { title }),
  /** Selector nativo de una carpeta; `null` si se cancela. */
  /** Selector nativo de una acción guardada (.toml o .json); `null` si se cancela. */
  pickActionFile: (title?: string) => invoke<string | null>("pick_action_file", { title, filter: t("dialog.filter.action") }),
  pickFolder: (title?: string) => invoke<string | null>("pick_folder", { title }),
  /** Selector nativo de una imagen PNG o JPEG; `null` si se cancela. */
  pickImageFile: (title?: string) => invoke<string | null>("pick_image_file", { title, filter: t("dialog.filter.image") }),
  /** Selector nativo de imágenes, texto y documentos de Office para «Crear PDF». */
  pickSourceFiles: (title?: string) => invoke<string[]>("pick_source_files", { title, filter: t("dialog.filter.source") }),
};

export const events = {
  onDocumentChanged: (f: (e: DocumentChanged) => void): Promise<UnlistenFn> =>
    listen<DocumentChanged>("document-changed", (e) => f(e.payload)),
  onOpenFiles: (f: (paths: string[]) => void): Promise<UnlistenFn> =>
    listen<string[]>("open-files", (e) => f(e.payload)),
};

/** URL de una tesela. En Windows (WebView2) los protocolos propios son http://<esquema>.localhost. */
export function tileUrl(
  docId: DocId,
  page: number,
  zoom: number,
  col: number,
  row: number,
  revision: number,
  windows = typeof navigator !== "undefined" && navigator.userAgent.includes("Windows"),
): string {
  const base = windows ? "http://pliego.localhost" : "pliego://localhost";
  return `${base}/tile/${docId}/${page}/${zoom}/${col}/${row}?rev=${revision}`;
}

export function isApiError(e: unknown): e is ApiError {
  return typeof e === "object" && e !== null && "code" in e && "message" in e;
}
