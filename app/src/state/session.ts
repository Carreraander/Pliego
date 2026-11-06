/**
 * Sesión: la app recuerda qué pestañas hay abiertas (ruta y página, nunca contraseñas) y, si la vez anterior se cerró de
 * golpe, Inicio ofrece reabrirlas y copiar un informe del fallo (`pliego_appdata::session`). Nada sale del equipo.
 */
import { createEffect, createRoot, on } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { setRecoverable, type Recoverable } from "../home/recovery";
import { activeId, currentPageIndex, docs } from "./documents";
import { goToPage } from "./navigation";

export interface SessionDoc { path: string; page: number }
interface PreviousRun { crashed: boolean; docs: SessionDoc[] }

const fileName = (p: string) => p.split(/[\\/]/).pop() ?? p;

/** Lista que se guarda: el orden de las pestañas y la última página vista de cada una. Pura, para probarla. */
export function sessionDocs(paths: string[], pages: Map<string, number>): SessionDoc[] {
  return paths.map((path) => ({ path, page: pages.get(path) ?? 0 }));
}

/** Tras abrir lo que pidió el sistema, mira cómo acabó la vez anterior y empieza a guardar la sesión. Con `reopen`
 *  (ajuste «Reabrir las pestañas al iniciar») vuelve a abrir directamente las pestañas de un cierre normal. */
export async function installSession(open: (path: string) => Promise<void>, reopen = false): Promise<() => void> {
  const previous = await invoke<PreviousRun | null>("session_previous").catch(() => null);
  if (reopen && previous && !previous.crashed && previous.docs.length && docs().length === 0) {
    for (const d of previous.docs) {
      try {
        await open(d.path);
        if (d.page > 0) goToPage(d.page);
      } catch { /* si uno ya no abre, se siguen los demás */ }
    }
  } else if (previous?.crashed && previous.docs.length && docs().length === 0) {
    const pageOf = new Map(previous.docs.map((d) => [d.path, d.page]));
    const list: Recoverable[] = previous.docs.map((d) => ({ id: d.path, title: fileName(d.path) }));
    setRecoverable(list, {
      restore: async (chosen) => {
        for (const r of chosen) {
          await open(r.id);
          const page = pageOf.get(r.id) ?? 0;
          if (page > 0) goToPage(page);
        }
      },
      discard: () => undefined,
      report: () => invoke<string>("crash_report"),
    });
  }
  const pages = new Map<string, number>();
  return createRoot((dispose) => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const save = () => {
      const list = docs();
      const live = new Set(list.map((d) => d.path));
      for (const p of [...pages.keys()]) if (!live.has(p)) pages.delete(p);
      void invoke("session_save", { docs: sessionDocs(list.map((d) => d.path), pages) }).catch(() => undefined);
    };
    createEffect(on([docs, activeId, currentPageIndex], () => {
      const d = docs().find((x) => x.id === activeId());
      if (d) pages.set(d.path, currentPageIndex());
      // Sin prisa: desplazarse cambia la página muchas veces por segundo.
      clearTimeout(timer);
      timer = setTimeout(save, 800);
    }, { defer: previous?.crashed ?? false }));
    return () => { clearTimeout(timer); dispose(); };
  });
}
