/**
 * Gestor central de atajos (docs/design §6): una sola tabla, sin conflictos y configurable. Los atajos del armazón
 * (F4, F6, F9, Alt+1…9, Ctrl+K, Ctrl+Mayús+A) los trata `shell/shortcuts.ts`; aquí se listan como «externos» para la hoja
 * de atajos y para detectar conflictos al reasignar.
 */
import { createSignal } from "solid-js";
import { locale } from "../i18n";
import { settings, updateSettings } from "../settings/store";
import { displayCombo, eventCombo, isPlainKey, normalizeCombo } from "./keys";

export interface ShortcutDef {
  /** `<area>.<orden>`, p. ej. `file.open`. */
  id: string;
  /** Texto ya traducido (o función, para que siga al idioma). */
  title: string | (() => string);
  /** Combinación por defecto, p. ej. `Ctrl+Shift+A` o `H`; vacío = sin atajo. */
  keys: string;
  /** `global` siempre; `canvas`: letras sueltas, solo con el foco fuera de un campo de texto. */
  scope?: "global" | "canvas";
  /** Sección de la hoja de atajos. */
  section: ShortcutSection;
  /** Si falta, otro código (el armazón, un flujo) ya lo trata: solo se lista. */
  run?: () => void;
  /** No se ejecuta si devuelve `false`. */
  when?: () => boolean;
}
export type ShortcutSection = "file" | "navigation" | "zoom" | "search" | "panels" | "modes" | "tools" | "edit";

const [defs, setDefs] = createSignal<ShortcutDef[]>([]);
export const shortcutDefs = defs;

export function registerShortcut(def: ShortcutDef): () => void {
  setDefs((all) => [...all.filter((d) => d.id !== def.id), def]);
  return () => setDefs((all) => all.filter((d) => d !== def));
}

export const shortcutTitle = (d: ShortcutDef): string => (typeof d.title === "function" ? d.title() : d.title);

/** Combinación efectiva: la elegida por el usuario o la de por defecto. */
export function effectiveKeys(def: ShortcutDef): string {
  const own = settings().shortcuts[def.id];
  return normalizeCombo(own !== undefined ? own : def.keys);
}

/** Grupos de órdenes que comparten combinación (conflictos). */
export function conflicts(): { combo: string; ids: string[] }[] {
  const by = new Map<string, string[]>();
  for (const d of defs()) {
    const k = effectiveKeys(d);
    if (k) by.set(k, [...(by.get(k) ?? []), d.id]);
  }
  return [...by].filter(([, ids]) => ids.length > 1).map(([combo, ids]) => ({ combo, ids }));
}

/** Reasigna un atajo. Devuelve el título de la orden que ya usa esa combinación (y no cambia nada), o `null` si se aplicó. */
export function assignShortcut(id: string, combo: string): string | null {
  const next = normalizeCombo(combo);
  if (next) {
    const other = defs().find((d) => d.id !== id && effectiveKeys(d) === next);
    if (other) return shortcutTitle(other);
  }
  const def = defs().find((d) => d.id === id);
  const rest = { ...settings().shortcuts };
  if (def && normalizeCombo(def.keys) === next) delete rest[id]; else rest[id] = next;
  updateSettings("shortcuts", rest as never);
  return null;
}

export function resetShortcut(id: string): void {
  const rest = { ...settings().shortcuts };
  delete rest[id];
  updateSettings("shortcuts", rest as never);
}

/** Texto para mostrar el atajo de una orden en el idioma activo. */
export const shortcutLabel = (def: ShortcutDef): string => displayCombo(effectiveKeys(def), locale());

const editable = (el: EventTarget | null): boolean => {
  const n = el as HTMLElement | null;
  return !!n && (n.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(n.tagName));
};

/** Busca la orden de un evento. Pura, para probarla sin DOM. */
export function matchShortcut(e: Pick<KeyboardEvent, "key" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey">, inField: boolean, list: ShortcutDef[] = defs()): ShortcutDef | null {
  const combo = eventCombo(e);
  if (!combo) return null;
  for (const d of list) {
    if (!d.run || effectiveKeys(d) !== combo) continue;
    if ((d.scope === "canvas" || isPlainKey(combo)) && inField) continue;
    return d;
  }
  return null;
}

let installed = false;
/** Instala el oyente global (una vez). Devuelve cómo quitarlo. */
export function installShortcuts(): () => void {
  if (installed) return () => undefined;
  installed = true;
  const onKey = (e: KeyboardEvent) => {
    if (e.defaultPrevented || e.isComposing) return;
    const hit = matchShortcut(e, editable(e.target));
    if (!hit || (hit.when && !hit.when())) return;
    e.preventDefault();
    hit.run?.();
  };
  addEventListener("keydown", onKey);
  return () => { removeEventListener("keydown", onKey); installed = false; };
}

/** Solo para pruebas. */
export function resetShortcuts(): void { setDefs([]); }
