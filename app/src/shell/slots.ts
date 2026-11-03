/**
 * Ranuras del armazón: la API con la que otros flujos se montan dentro sin tocar `shell/`.
 * (docs/app-integration.md, sección «Interfaz»)
 *
 *  - `registerPanel`        pestaña en el raíl + contenido del panel lateral
 *  - `openInspector`        contenido del inspector (propiedades o panel de tarea)
 *  - `showNotice`           barra de aviso del documento (una sola visible, por prioridad)
 *  - `registerStatusItem`   elemento de la barra de estado
 *  - `registerTitleItem`    hueco de la barra de título (pastilla de actualización)
 *  - `registerModeToolbar`  barra de herramientas de un modo
 *  - `setPaletteOpener`     paleta de órdenes (la implementa W20)
 */
import { batch, createSignal, type Component } from "solid-js";
import type { NoticeAction, NoticeTone } from "../ui";

function makeRegistry<T extends { id: string }>() {
  const [items, setItems] = createSignal<T[]>([]);
  return {
    items,
    add(item: T): () => void {
      setItems((l) => [...l.filter((i) => i.id !== item.id), item]);
      return () => setItems((l) => l.filter((i) => i !== item));
    },
  };
}

// ── Paneles del raíl ─────────────────────────────────────────────────────────────────
export interface PanelDef {
  /** Único, `<area>.<panel>` (p. ej. `viewer.thumbnails`). */
  id: string;
  /** Texto ya traducido: nombre accesible y tooltip. */
  title: string;
  /** Nombre de icono (`app/src/ui/Icon`). */
  icon: string;
  /** Menor = más arriba en el raíl. */
  order?: number;
  shortcut?: string;
  /** Contenido; recibe el documento activo (o `null` si no hay). */
  component: Component<{ docId: string | null }>;
  /** Solo se muestra si hay documento (por defecto sí). */
  needsDocument?: boolean;
}
const panelsReg = makeRegistry<PanelDef>();
export const registerPanel = (def: PanelDef) => panelsReg.add(def);
export const panels = () => [...panelsReg.items()].sort((a, b) => (a.order ?? 100) - (b.order ?? 100));

// ── Inspector ────────────────────────────────────────────────────────────────────────
export interface InspectorContent { component: Component<any>; props: Record<string, unknown>; key: number } // eslint-disable-line @typescript-eslint/no-explicit-any
const [inspector, setInspector] = createSignal<InspectorContent | null>(null);
let inspectorKey = 0;
export { inspector };
/** Abre el inspector con un componente (panel de tarea, propiedades…). Sustituye al anterior. */
export function openInspector<P extends Record<string, unknown>>(component: Component<P>, props: P = {} as P): void {
  setInspector({ component, props, key: ++inspectorKey });
}
let stashed: InspectorContent | null = null;
export function closeInspector(): void { setInspector(null); }
/** F9: oculta el inspector recordando su contenido, o lo recupera. */
export function toggleInspector(): void {
  const current = inspector();
  if (current) { stashed = current; setInspector(null); } else if (stashed) { setInspector(stashed); }
}

// ── Avisos del documento ─────────────────────────────────────────────────────────────
/** Orden de prioridad de §5.2: reparado > firmado > protegido > formulario > PDF/A > modo restringido. */
export const NOTICE_PRIORITY = ["repaired", "signed", "protected", "form", "pdfa", "restricted", "other"] as const;
export type NoticeKind = (typeof NOTICE_PRIORITY)[number];
export interface DocNotice { kind?: NoticeKind; id?: string; tone?: NoticeTone; text: string; actions?: NoticeAction[]; dismissible?: boolean }
interface StoredNotice extends DocNotice { id: string }
const [notices, setNotices] = createSignal<Record<string, StoredNotice[]>>({});
let noticeSeq = 0;

/** Muestra un aviso en el documento `docId`. Con el mismo `id` lo reemplaza. Devuelve cómo quitarlo. */
export function showNotice(docId: string, notice: DocNotice): () => void {
  const id = notice.id ?? `n${++noticeSeq}`;
  setNotices((all) => ({ ...all, [docId]: [...(all[docId] ?? []).filter((n) => n.id !== id), { ...notice, id }] }));
  return () => dismissNotice(docId, id);
}
export function dismissNotice(docId: string, id: string): void {
  setNotices((all) => ({ ...all, [docId]: (all[docId] ?? []).filter((n) => n.id !== id) }));
}
export function clearNotices(docId: string): void {
  setNotices((all) => Object.fromEntries(Object.entries(all).filter(([k]) => k !== docId)));
}
/** El aviso visible de un documento: el de mayor prioridad (y, a igual prioridad, el más reciente). */
export function noticeFor(docId: string | null): StoredNotice | null {
  if (!docId) return null;
  const list = notices()[docId] ?? [];
  return pickNotice(list);
}
export function pickNotice<T extends { kind?: NoticeKind }>(list: T[]): T | null {
  let best: T | null = null;
  let bestRank = Infinity;
  for (const n of list) {
    const rank = NOTICE_PRIORITY.indexOf(n.kind ?? "other");
    if (rank <= bestRank) { best = n; bestRank = rank; }
  }
  return best;
}

// ── Barra de estado ──────────────────────────────────────────────────────────────────
export interface StatusItem {
  id: string;
  /** `start` (página y papel), `center` (tareas, estado del documento) o `end` (zoom y vista). */
  slot: "start" | "center" | "end";
  order?: number;
  /** Contenido textual o componente. */
  text?: () => string;
  component?: Component;
  /** Nombre accesible; por defecto, el texto. */
  label?: string;
  /** Si existe, el elemento es un botón. */
  onClick?: () => void;
  tone?: NoticeTone;
}
const statusReg = makeRegistry<StatusItem>();
export const registerStatusItem = (item: StatusItem) => statusReg.add(item);
export const statusItems = (slot: StatusItem["slot"]) => statusReg.items().filter((i) => i.slot === slot).sort((a, b) => (a.order ?? 100) - (b.order ?? 100));

// ── Hueco de la barra de título (antes de los botones de ventana) ───────────────────────
export interface TitleItem { id: string; order?: number; component: Component }
const titleReg = makeRegistry<TitleItem>();
export const registerTitleItem = (item: TitleItem) => titleReg.add(item);
export const titleItems = () => [...titleReg.items()].sort((a, b) => (a.order ?? 100) - (b.order ?? 100));

// ── Modos y barras de herramientas del modo ───────────────────────────────────────────
export interface ModeDef {
  id: string;
  title: string;
  icon?: string;
  order?: number;
  /** Barra del modo (derecha de la barra de modos) y, opcionalmente, contenido de inspector por defecto. */
  toolbar?: Component<{ docId: string | null }>;
  needsDocument?: boolean;
}
const modesReg = makeRegistry<ModeDef>();
export const registerMode = (def: ModeDef) => modesReg.add(def);
export const modes = () => [...modesReg.items()].sort((a, b) => (a.order ?? 100) - (b.order ?? 100));
const [activeModeId, setActiveModeId] = createSignal<string | null>(null);
export const activeMode = () => modes().find((m) => m.id === activeModeId()) ?? modes()[0] ?? null;
export function setMode(id: string): void { setActiveModeId(id); }

// ── Paleta de órdenes (la hace W20) ─────────────────────────────────────────────────
let paletteOpener: ((query?: string) => void) | null = null;
export function setPaletteOpener(fn: ((query?: string) => void) | null): void { paletteOpener = fn; }
export function openPalette(query?: string): boolean { paletteOpener?.(query); return paletteOpener !== null; }
let toolsOpener: (() => void) | null = null;
export function setToolsOpener(fn: (() => void) | null): void { toolsOpener = fn; }
export function openTools(): boolean { toolsOpener?.(); return toolsOpener !== null; }

/** Solo para pruebas. */
export function resetSlots(): void {
  batch(() => { setInspector(null); setNotices({}); setActiveModeId(null); });
}
