import { onCleanup } from "solid-js";
import { setSideOpen, sideOpen } from "./layout";
import { modes, openPalette, openTools, setMode, toggleInspector } from "./slots";

/** Orden de las regiones para `F6` (§7: barra de título → modos → raíl → panel → lienzo → inspector → estado). */
export const REGION_ORDER = ["titlebar", "modebar", "rail", "sidepanel", "canvas", "inspector", "statusbar"] as const;

/** Siguiente región visible a partir de la actual (o la primera). `dir` = 1 adelante, -1 atrás. */
export function nextRegion(current: string | null, present: string[], dir: 1 | -1): string | null {
  const list = REGION_ORDER.filter((r) => present.includes(r));
  if (!list.length) return null;
  const i = current ? list.indexOf(current as (typeof REGION_ORDER)[number]) : -1;
  if (i < 0) return list[dir === 1 ? 0 : list.length - 1] ?? null;
  return list[(i + dir + list.length) % list.length] ?? null;
}

function focusRegion(name: string): void {
  const el = document.querySelector<HTMLElement>(`[data-region="${name}"]`);
  if (!el) return;
  const target = el.querySelector<HTMLElement>('[tabindex="0"], [role="tab"][aria-selected="true"], button:not([disabled]), input, [href]') ?? el;
  target.focus();
}

/** Atajos globales del armazón: F4 panel lateral, F6/Mayús+F6 regiones, F9 inspector, Alt+1…9 modos, Ctrl+K paleta, Ctrl+Mayús+A herramientas. */
export function installShellShortcuts(): void {
  const onKey = (e: KeyboardEvent) => {
    if (e.key === "F4" && !e.ctrlKey && !e.altKey) { e.preventDefault(); setSideOpen(!sideOpen()); return; }
    if (e.key === "F6" && !e.ctrlKey && !e.altKey) {
      e.preventDefault();
      const present = Array.from(document.querySelectorAll<HTMLElement>("[data-region]")).map((r) => r.dataset.region!);
      const current = (document.activeElement as HTMLElement | null)?.closest<HTMLElement>("[data-region]")?.dataset.region ?? null;
      const next = nextRegion(current, present, e.shiftKey ? -1 : 1);
      if (next) focusRegion(next);
      return;
    }
    if (e.key === "F9") { e.preventDefault(); toggleInspector(); return; }
    if (e.altKey && !e.ctrlKey && !e.shiftKey && /^[1-9]$/.test(e.key)) {
      const m = modes()[Number(e.key) - 1];
      if (m) { e.preventDefault(); setMode(m.id); }
      return;
    }
    if (e.ctrlKey && e.shiftKey && !e.altKey && e.key.toLowerCase() === "a") { e.preventDefault(); openTools(); return; }
    if (e.ctrlKey && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "k") { e.preventDefault(); openPalette(); }
  };
  addEventListener("keydown", onKey);
  onCleanup(() => removeEventListener("keydown", onKey));
}
