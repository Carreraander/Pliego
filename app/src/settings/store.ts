/**
 * Ajustes de la app: un objeto con esquema versionado, con efecto inmediato y persistido (docs/design §3.5).
 * En Windows vive en `%APPDATA%\Pliego\settings.json` (comandos Tauri `settings`); sin Tauri (desarrollo, pruebas) usa
 * `localStorage`. Los campos desconocidos se conservan al guardar, para no perder ajustes de una versión posterior.
 */
import { createSignal } from "solid-js";

export const SETTINGS_VERSION = 1;

export interface Settings {
  version: number;
  general: { language: "auto" | "es" | "en"; theme: "auto" | "light" | "dark"; density: "comfortable" | "compact" | "touch"; reopenTabs: boolean; greeting: boolean };
  viewer: { defaultZoom: "fit-width" | "fit-page" | "100"; smoothScroll: boolean; nightReading: boolean };
  accessibility: { reduceMotion: "auto" | "on" | "off"; textScale: number; highContrast: "auto" | "on" | "off" };
  /** Atajos cambiados por el usuario: id de la orden → combinación («» = sin atajo). */
  shortcuts: Record<string, string>;
  /** Herramientas fijadas como modo en la barra, en orden. */
  modes: { pinned: string[] };
  home: { quickActions: string[] };
  /** Versión cuyas novedades ya se mostraron en Inicio. */
  seenVersion: string;
}

export const DEFAULT_QUICK_ACTIONS = ["open", "merge", "ocr", "sign", "compress", "convert"];

export const DEFAULT_SETTINGS: Settings = {
  version: SETTINGS_VERSION,
  general: { language: "auto", theme: "auto", density: "comfortable", reopenTabs: false, greeting: true },
  viewer: { defaultZoom: "fit-width", smoothScroll: true, nightReading: false },
  accessibility: { reduceMotion: "auto", textScale: 1, highContrast: "auto" },
  shortcuts: {},
  modes: { pinned: [] },
  home: { quickActions: DEFAULT_QUICK_ACTIONS },
  seenVersion: "",
};

/** Dónde se guarda. W20 cambia el de `localStorage` por el de Tauri al arrancar dentro de la app. */
export interface SettingsBackend {
  load(): Promise<unknown>;
  save(value: unknown): Promise<void>;
}

const KEY = "pliego.settings";
export const localBackend: SettingsBackend = {
  async load() { try { const raw = localStorage.getItem(KEY); return raw ? JSON.parse(raw) : null; } catch { return null; } },
  async save(value) { try { localStorage.setItem(KEY, JSON.stringify(value)); } catch { /* sin almacenamiento */ } },
};

let backend: SettingsBackend = localBackend;
export function setSettingsBackend(b: SettingsBackend): void { backend = b; }

const isObject = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);

/** Mezcla `raw` sobre los valores por defecto sección a sección, ignorando tipos incorrectos. */
export function migrate(raw: unknown): Settings {
  const out: Settings = structuredClone(DEFAULT_SETTINGS);
  if (!isObject(raw)) return out;
  for (const [key, def] of Object.entries(DEFAULT_SETTINGS) as [keyof Settings, unknown][]) {
    const value = raw[key];
    if (value === undefined || key === "version") continue;
    if (isObject(def) && isObject(value) && key !== "shortcuts") {
      const section = out[key] as unknown as Record<string, unknown>;
      for (const [k, d] of Object.entries(def)) if (typeof value[k] === typeof d && (!Array.isArray(d) || Array.isArray(value[k]))) section[k] = value[k];
    } else if (key === "shortcuts" && isObject(value)) {
      out.shortcuts = Object.fromEntries(Object.entries(value).filter(([, v]) => typeof v === "string")) as Record<string, string>;
    } else if (typeof value === typeof def) (out as unknown as Record<string, unknown>)[key] = value;
  }
  for (const [k, v] of Object.entries(raw)) if (!(k in out)) (out as unknown as Record<string, unknown>)[k] = v;
  out.version = SETTINGS_VERSION;
  return out;
}

const [settings, setSettings] = createSignal<Settings>(structuredClone(DEFAULT_SETTINGS));
export { settings };

let saving: Promise<void> = Promise.resolve();
function persist(): void { const snapshot = settings(); saving = saving.then(() => backend.save(snapshot)).catch(() => undefined); }
/** Espera a que terminen los guardados pendientes (pruebas y cierre). */
export const flushSettings = (): Promise<void> => saving;

export async function loadSettings(): Promise<Settings> {
  let raw: unknown = null;
  try { raw = await backend.load(); } catch { /* se usan los valores por defecto */ }
  setSettings(migrate(raw));
  return settings();
}

/** Cambia una sección con efecto inmediato y la persiste (las secciones se mezclan; `shortcuts` se sustituye entero). */
export function updateSettings<K extends keyof Settings>(key: K, patch: Settings[K] extends object ? Partial<Settings[K]> : Settings[K]): void {
  setSettings((s) => ({ ...s, [key]: key !== "shortcuts" && isObject(s[key]) && !Array.isArray(s[key]) && isObject(patch) ? { ...(s[key] as Record<string, unknown>), ...(patch as Record<string, unknown>) } : patch }));
  persist();
}

/** Restablece todo (Ajustes > Avanzado). */
export function resetSettings(): void { setSettings(structuredClone(DEFAULT_SETTINGS)); persist(); }
