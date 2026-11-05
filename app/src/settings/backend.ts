import { invoke } from "@tauri-apps/api/core";
import { createEffect, createRoot } from "solid-js";
import { detectLocale, setLocale } from "../i18n";
import { applyDensity, applyTheme } from "../shell";
import { isTauri } from "../shell/platform";
import { loadSettings, setSettingsBackend, settings, type SettingsBackend } from "./store";

/** Ajustes en `%APPDATA%\Pliego\settings.json` (comandos `get_settings`/`set_settings`). */
export const tauriBackend: SettingsBackend = {
  load: () => invoke<unknown>("get_settings"),
  save: (value) => invoke<void>("set_settings", { value }),
};

/** Aplica los ajustes al documento (tema, densidad, idioma, movimiento y foco) cada vez que cambian. */
export function applySettings(): () => void {
  return createRoot((dispose) => {
    createEffect(() => {
      const g = settings().general;
      applyTheme(g.theme === "auto" ? "system" : g.theme);
      applyDensity(g.density);
      setLocale(g.language === "auto" ? detectLocale(navigator.language) : g.language);
    });
    createEffect(() => {
      const el = document.documentElement;
      const a = settings().accessibility;
      if (a.reduceMotion === "auto") el.removeAttribute("data-reduce-motion"); else el.setAttribute("data-reduce-motion", a.reduceMotion);
      if (a.strongFocus) el.setAttribute("data-strong-focus", ""); else el.removeAttribute("data-strong-focus");
      if (settings().viewer.nightReading) el.setAttribute("data-night-reading", ""); else el.removeAttribute("data-night-reading");
    });
    return dispose;
  });
}

/** Arranque: elige el almacén (Tauri o navegador), carga y empieza a aplicar. */
export async function initSettings(): Promise<() => void> {
  if (isTauri()) setSettingsBackend(tauriBackend);
  await loadSettings();
  return applySettings();
}
