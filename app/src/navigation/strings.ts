import { locale, type Locale } from "../i18n";

const dict = {
  es: {
    "nav.palette": "Paleta de órdenes", "nav.palette.placeholder": "Busca una herramienta, documento, página o ajuste…", "nav.palette.none": "Sin resultados para «{q}»",
    "nav.palette.hint": "↑↓ moverse · Intro ejecutar · Tab ampliar · Esc cerrar", "nav.group.commands": "Órdenes y herramientas", "nav.group.documents": "Documentos abiertos",
    "nav.group.recents": "Recientes", "nav.group.pages": "Ir a la página", "nav.group.text": "Buscar en el documento", "nav.group.settings": "Ajustes",
    "nav.goto": "Ir a la página {n}", "nav.gotoOf": "Ir a la página {n} de {total}", "nav.searchText": "Buscar «{q}» en el documento", "nav.switchTo": "Cambiar a {title}",
    "nav.tools": "Todas las herramientas", "nav.tools.search": "Buscar herramientas…", "nav.tools.none": "Ninguna herramienta coincide con «{q}»",
    "nav.tools.pin": "Fijar en la barra", "nav.tools.unpin": "Soltar de la barra", "nav.tools.moveLeft": "Mover a la izquierda", "nav.tools.moveRight": "Mover a la derecha",
    "nav.tools.soon": "próximamente", "nav.tools.open": "Abrir {title}", "nav.pinned": "Fijada en la barra",
    "nav.mode.soon": "Esta herramienta llegará en una próxima versión.", "nav.mode.open": "Abrir {title}",
    "nav.shortcuts": "Atajos de teclado", "nav.shortcuts.search": "Filtrar atajos…", "nav.shortcuts.close": "Cerrar",
    "nav.shortcuts.conflict": "Esa combinación ya la usa «{title}».", "nav.shortcuts.edit": "Cambiar el atajo de {title}", "nav.shortcuts.press": "Pulsa la nueva combinación",
    "nav.shortcuts.reset": "Restablecer", "nav.shortcuts.none": "Sin atajo", "nav.shortcuts.customized": "personalizado",
    "nav.section.file": "Archivo", "nav.section.navigation": "Navegación", "nav.section.zoom": "Zoom y vista", "nav.section.search": "Buscar", "nav.section.panels": "Paneles",
    "nav.section.modes": "Modos", "nav.section.tools": "Herramientas de modo", "nav.section.edit": "Edición",
    "nav.cmd.tools": "Todas las herramientas", "nav.cmd.shortcuts": "Hoja de atajos", "nav.cmd.palette": "Paleta de órdenes",
  },
  en: {
    "nav.palette": "Command palette", "nav.palette.placeholder": "Search a tool, document, page or setting…", "nav.palette.none": "No results for “{q}”",
    "nav.palette.hint": "↑↓ move · Enter run · Tab expand · Esc close", "nav.group.commands": "Commands and tools", "nav.group.documents": "Open documents",
    "nav.group.recents": "Recent", "nav.group.pages": "Go to page", "nav.group.text": "Search the document", "nav.group.settings": "Settings",
    "nav.goto": "Go to page {n}", "nav.gotoOf": "Go to page {n} of {total}", "nav.searchText": "Search “{q}” in the document", "nav.switchTo": "Switch to {title}",
    "nav.tools": "All tools", "nav.tools.search": "Search tools…", "nav.tools.none": "No tool matches “{q}”",
    "nav.tools.pin": "Pin to the bar", "nav.tools.unpin": "Unpin from the bar", "nav.tools.moveLeft": "Move left", "nav.tools.moveRight": "Move right",
    "nav.tools.soon": "coming soon", "nav.tools.open": "Open {title}", "nav.pinned": "Pinned to the bar",
    "nav.mode.soon": "This tool will arrive in a future version.", "nav.mode.open": "Open {title}",
    "nav.shortcuts": "Keyboard shortcuts", "nav.shortcuts.search": "Filter shortcuts…", "nav.shortcuts.close": "Close",
    "nav.shortcuts.conflict": "That combination is already used by “{title}”.", "nav.shortcuts.edit": "Change the shortcut for {title}", "nav.shortcuts.press": "Press the new combination",
    "nav.shortcuts.reset": "Reset", "nav.shortcuts.none": "No shortcut", "nav.shortcuts.customized": "customized",
    "nav.section.file": "File", "nav.section.navigation": "Navigation", "nav.section.zoom": "Zoom and view", "nav.section.search": "Search", "nav.section.panels": "Panels",
    "nav.section.modes": "Modes", "nav.section.tools": "Mode tools", "nav.section.edit": "Editing",
    "nav.cmd.tools": "All tools", "nav.cmd.shortcuts": "Shortcut sheet", "nav.cmd.palette": "Command palette",
  },
} as const satisfies Record<Locale, Record<string, string>>;

export type NavKey = keyof (typeof dict)["es"];

export function nt(key: NavKey, vars: Record<string, string | number> = {}): string {
  const text: string = dict[locale()][key] ?? dict.es[key];
  return text.replace(/\{(\w+)\}/g, (_, n: string) => String(vars[n] ?? `{${n}}`));
}
