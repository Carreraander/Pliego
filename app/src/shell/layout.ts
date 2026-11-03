import { createSignal } from "solid-js";

/** Medidas del armazón (docs/design/ux-y-navegacion.md §2). Las alturas fijas salen de tokens.css. */
export const SIDEPANEL = { min: 200, max: 480, default: 264 } as const;
export const INSPECTOR = { min: 280, max: 480, default: 320 } as const;
export const BREAKPOINT_MEDIUM = 1100;
export const BREAKPOINT_NARROW = 900;

export type LayoutMode = "wide" | "medium" | "narrow";

/** < 1100 px: los modos se pliegan a un desplegable y la barra del modo pasa a iconos. < 900 px: paneles superpuestos. */
export function layoutMode(width: number): LayoutMode {
  return width < BREAKPOINT_NARROW ? "narrow" : width < BREAKPOINT_MEDIUM ? "medium" : "wide";
}

export function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, Math.round(value)));
}

/** Persistencia tolerante: sin `localStorage` (modo privado, pruebas) simplemente no recuerda. */
export function load<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(`pliego.shell.${key}`);
    return raw === null ? fallback : (JSON.parse(raw) as T);
  } catch { return fallback; }
}
export function save(key: string, value: unknown): void {
  try { localStorage.setItem(`pliego.shell.${key}`, JSON.stringify(value)); } catch { /* sin almacenamiento */ }
}

const [windowWidth, setWindowWidth] = createSignal(typeof innerWidth === "number" ? innerWidth : 1280);
if (typeof addEventListener === "function") addEventListener("resize", () => setWindowWidth(innerWidth));
export { windowWidth };
export const mode = () => layoutMode(windowWidth());

// ── Panel lateral: ancho recordado por panel ────────────────────────────────────────────
const [sideWidths, setSideWidths] = createSignal<Record<string, number>>(load("sideWidths", {}));
const [activePanel, setActivePanelRaw] = createSignal<string | null>(load("activePanel", null));
const [sideOpen, setSideOpenRaw] = createSignal<boolean>(load("sideOpen", true));

export const sidePanelWidth = (panelId: string | null) => (panelId && sideWidths()[panelId]) || SIDEPANEL.default;
export function setSidePanelWidth(panelId: string, width: number): void {
  setSideWidths((w) => { const n = { ...w, [panelId]: clamp(width, SIDEPANEL.min, SIDEPANEL.max) }; save("sideWidths", n); return n; });
}
export { activePanel, sideOpen };
export function setActivePanel(id: string | null): void { setActivePanelRaw(id); save("activePanel", id); }
export function setSideOpen(open: boolean): void { setSideOpenRaw(open); save("sideOpen", open); }
/** Clic en el activo pliega el panel; en otro, lo abre (§2: raíl). */
export function toggleRail(id: string): void {
  if (sideOpen() && activePanel() === id) setSideOpen(false);
  else { setActivePanel(id); setSideOpen(true); }
}

// ── Inspector: ancho recordado ──────────────────────────────────────────────────────────
const [inspectorWidthRaw, setInspectorWidthRaw] = createSignal<number>(load("inspectorWidth", INSPECTOR.default));
export const inspectorWidth = inspectorWidthRaw;
export function setInspectorWidth(width: number): void { const w = clamp(width, INSPECTOR.min, INSPECTOR.max); setInspectorWidthRaw(w); save("inspectorWidth", w); }

// ── Tema y densidad (Ajustes los cambia; aquí solo se aplican a <html>) ────────────────
export type ThemeChoice = "system" | "light" | "dark";
export type DensityChoice = "compact" | "comfortable" | "touch";
export function applyTheme(theme: ThemeChoice): void {
  const el = document.documentElement;
  if (theme === "system") el.removeAttribute("data-theme"); else el.setAttribute("data-theme", theme);
}
export function applyDensity(density: DensityChoice): void {
  const el = document.documentElement;
  if (density === "comfortable") el.removeAttribute("data-density"); else el.setAttribute("data-density", density);
}
