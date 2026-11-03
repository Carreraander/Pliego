import { locale } from "../i18n";
import { registerShortcut, type ShortcutDef, type ShortcutSection } from "./shortcuts";

/** Tabla de §6. Los atajos con `run` los trata esta carpeta; el resto los tratan el armazón o el visor y aquí solo se listan. */
type Row = [id: string, section: ShortcutSection, keys: string, es: string, en: string];
export const REFERENCE: Row[] = [
  ["file.open", "file", "Ctrl+O", "Abrir", "Open"], ["file.save", "file", "Ctrl+S", "Guardar", "Save"],
  ["file.saveAs", "file", "Ctrl+Shift+S", "Guardar como", "Save as"], ["file.print", "file", "Ctrl+P", "Imprimir", "Print"],
  ["file.closeTab", "file", "Ctrl+W", "Cerrar pestaña", "Close tab"], ["file.reopenTab", "file", "Ctrl+Shift+T", "Reabrir pestaña cerrada", "Reopen closed tab"],
  ["file.properties", "file", "Ctrl+D", "Propiedades del documento", "Document properties"], ["file.settings", "file", "Ctrl+,", "Ajustes", "Settings"],
  ["nav.nextTab", "navigation", "Ctrl+Tab", "Pestaña siguiente", "Next tab"], ["nav.prevTab", "navigation", "Ctrl+Shift+Tab", "Pestaña anterior", "Previous tab"],
  ["nav.goto", "navigation", "Ctrl+G", "Ir a la página", "Go to page"], ["nav.home", "navigation", "Home", "Primera página", "First page"], ["nav.end", "navigation", "End", "Última página", "Last page"],
  ["nav.pageUp", "navigation", "PageUp", "Página anterior", "Previous page"], ["nav.pageDown", "navigation", "PageDown", "Página siguiente", "Next page"],
  ["nav.back", "navigation", "Alt+ArrowLeft", "Atrás en la historia de saltos", "Back in jump history"], ["nav.forward", "navigation", "Alt+ArrowRight", "Adelante en la historia de saltos", "Forward in jump history"],
  ["zoom.in", "zoom", "Ctrl++", "Acercar", "Zoom in"], ["zoom.out", "zoom", "Ctrl+-", "Alejar", "Zoom out"], ["zoom.fitPage", "zoom", "Ctrl+0", "Ajustar a la página", "Fit page"],
  ["zoom.actual", "zoom", "Ctrl+1", "Tamaño real (100 %)", "Actual size (100%)"], ["zoom.fitWidth", "zoom", "Ctrl+2", "Ajustar al ancho", "Fit width"],
  ["view.rotate", "zoom", "Ctrl+R", "Girar la vista", "Rotate view"], ["view.present", "zoom", "F11", "Presentación", "Presentation"],
  ["search.find", "search", "Ctrl+F", "Buscar en el documento", "Find in document"], ["search.next", "search", "F3", "Siguiente coincidencia", "Next match"], ["search.prev", "search", "Shift+F3", "Coincidencia anterior", "Previous match"],
  ["shell.palette", "search", "Ctrl+K", "Paleta de órdenes", "Command palette"],
  ["shell.side", "panels", "F4", "Panel lateral", "Side panel"], ["shell.inspector", "panels", "F9", "Inspector", "Inspector"], ["shell.regions", "panels", "F6", "Recorrer regiones", "Cycle regions"],
  ["shell.tools", "panels", "Ctrl+Shift+A", "Todas las herramientas", "All tools"],
  ["shell.mode1", "modes", "Alt+1", "Modo 1 … Alt+9 modo 9 (según la barra)", "Mode 1 … Alt+9 mode 9 (by bar position)"],
  ["edit.undo", "edit", "Ctrl+Z", "Deshacer", "Undo"], ["edit.redo", "edit", "Ctrl+Y", "Rehacer", "Redo"], ["edit.delete", "edit", "Delete", "Eliminar", "Delete"],
  ["edit.copy", "edit", "Ctrl+C", "Copiar", "Copy"], ["edit.selectAll", "edit", "Ctrl+A", "Seleccionar todo", "Select all"],
];

/** Las filas que otros tratan: se listan en la hoja y cuentan para los conflictos, pero no tienen `run` aquí. */
export function registerReferenceShortcuts(own: Set<string>): () => void {
  const offs = REFERENCE.filter(([id]) => !own.has(id)).map(([id, section, keys, es, en]) => {
    const def: ShortcutDef = { id, section, keys, title: () => (locale() === "es" ? es : en) };
    return registerShortcut(def);
  });
  return () => offs.forEach((off) => off());
}
