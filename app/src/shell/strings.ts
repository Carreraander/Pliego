import { locale, type Locale } from "../i18n";

/** Textos del armazón (barra de título, modos, raíl, estado). */
const dict = {
  es: {
    "shell.appMenu": "Menú de la aplicación", "shell.documents": "Documentos abiertos", "shell.newTab": "Nueva pestaña",
    "shell.closeTab": "Cerrar {title}", "shell.unsaved": "con cambios sin guardar", "shell.search": "Buscar o ejecutar…",
    "shell.minimize": "Minimizar", "shell.maximize": "Maximizar", "shell.restore": "Restaurar", "shell.close": "Cerrar",
    "shell.modes": "Modos", "shell.modeTools": "Herramientas del modo", "shell.tools": "Herramientas", "shell.rail": "Paneles",
    "shell.sidePanel": "Panel lateral", "shell.inspector": "Inspector", "shell.status": "Barra de estado", "shell.canvas": "Documento",
    "shell.resizeSide": "Ancho del panel lateral", "shell.resizeInspector": "Ancho del inspector", "shell.currentMode": "Modo: {title}",
    "shell.titlebar": "Barra de título", "shell.closeSide": "Cerrar panel lateral",
  },
  en: {
    "shell.appMenu": "App menu", "shell.documents": "Open documents", "shell.newTab": "New tab",
    "shell.closeTab": "Close {title}", "shell.unsaved": "unsaved changes", "shell.search": "Search or run…",
    "shell.minimize": "Minimize", "shell.maximize": "Maximize", "shell.restore": "Restore", "shell.close": "Close",
    "shell.modes": "Modes", "shell.modeTools": "Mode tools", "shell.tools": "Tools", "shell.rail": "Panels",
    "shell.sidePanel": "Side panel", "shell.inspector": "Inspector", "shell.status": "Status bar", "shell.canvas": "Document",
    "shell.resizeSide": "Side panel width", "shell.resizeInspector": "Inspector width", "shell.currentMode": "Mode: {title}",
    "shell.titlebar": "Title bar", "shell.closeSide": "Close side panel",
  },
} as const satisfies Record<Locale, Record<string, string>>;

export type ShellKey = keyof (typeof dict)["es"];

export function st(key: ShellKey, vars: Record<string, string | number> = {}): string {
  const text: string = dict[locale()][key] ?? dict.es[key];
  return text.replace(/\{(\w+)\}/g, (_, n: string) => String(vars[n] ?? `{${n}}`));
}
