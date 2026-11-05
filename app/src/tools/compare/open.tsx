import { invoke } from "@tauri-apps/api/core";
import { createEffect, onCleanup } from "solid-js";
import { api } from "../../api";
import { closeScreen, openScreen } from "../../navigation/screen";
import { docs, type OpenDoc } from "../../state/documents";
import { passwordFor } from "../../state/passwords";
import CompareView, { openSideDoc, type SideDoc } from "./CompareView";
import { cmt } from "./strings";
import type { Comparison } from "./view";

export const COMPARE_SCREEN = "compare.view";

export interface SideBySideRequest { doc: OpenDoc; other: string; text: boolean; visual: boolean }

/** Pantalla de la vista lado a lado. Al cerrarse (por cualquier camino) suelta el segundo documento. */
function Host(p: { comparison: Comparison; a: OpenDoc; b: SideDoc }) {
  onCleanup(() => void api.closeDocument(p.b.id).catch(() => undefined));
  // Si se cierra el documento original mientras se mira la comparación, se cierra también la vista.
  createEffect(() => { if (!docs().some((d) => d.id === p.a.id)) closeScreen(); });
  return <CompareView comparison={p.comparison} a={p.a} b={p.b} onClose={closeScreen} />;
}

/**
 * Compara el documento abierto con `other` y abre la vista lado a lado. Lanza si la comparación falla; el segundo
 * documento solo se abre en el servicio (sin pestaña) y se cierra con la vista.
 */
export async function openSideBySide(r: SideBySideRequest): Promise<void> {
  const passwordA = passwordFor(r.doc.path);
  const passwordB = passwordFor(r.other);
  const comparison = await invoke<Comparison>("compare_view", { path: r.doc.path, other: r.other, passwordA, passwordB, text: r.text, visual: r.visual });
  const b = await openSideDoc(r.other, passwordB);
  openScreen({ id: COMPARE_SCREEN, title: () => cmt("view.tab"), component: () => <Host comparison={comparison} a={r.doc} b={b} /> });
}
