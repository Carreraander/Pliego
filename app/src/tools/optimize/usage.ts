import { invoke } from "@tauri-apps/api/core";
import { createResource } from "solid-js";
import { activeDoc } from "../../state/documents";

export interface SpaceUsage {
  total_bytes: number; images_bytes: number; fonts_bytes: number; content_bytes: number;
  other_streams_bytes: number; structure_bytes: number; image_count: number; object_count: number;
}

/** Análisis del tamaño del documento activo (en segundo plano; falla en silencio si el archivo está cifrado). */
export function createUsage() {
  return createResource(() => activeDoc()?.path ?? null, (path) => invoke<SpaceUsage>("analyze_pdf", { path, password: null }));
}
