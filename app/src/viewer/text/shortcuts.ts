import { locale } from "../../i18n";
import { registerShortcut } from "../../navigation/shortcuts";
import { currentScreen } from "../../navigation/screen";
import { activeMode } from "../../shell";
import { activeDoc } from "../../state/documents";
import { copySelection, hasSelection, selectAll } from "./state";

/** `true` si el foco está en un campo de texto (ahí Ctrl+C y Ctrl+A son del campo). */
const inField = (): boolean => {
  const el = document.activeElement as HTMLElement | null;
  return !!el && (el.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(el.tagName));
};

/** El visor está a la vista, en el modo Leer, y nadie más tiene el texto seleccionado. */
const reading = (): boolean => !!activeDoc() && currentScreen() === null && activeMode()?.id === "read" && !inField();

/** Copiar (Ctrl+C) y Seleccionar todo (Ctrl+A) sobre el texto de las páginas. */
export function registerTextShortcuts(): () => void {
  const es = () => locale() === "es";
  const offs = [
    registerShortcut({
      id: "edit.copy", section: "edit", keys: "Ctrl+C", title: () => (es() ? "Copiar" : "Copy"),
      // Si hay texto seleccionado en otro sitio de la ventana (un panel, el asistente), Ctrl+C es suyo.
      when: () => reading() && hasSelection(activeDoc()!) && !window.getSelection()?.toString(),
      run: () => void copySelection(activeDoc()!),
    }),
    registerShortcut({
      id: "edit.selectAll", section: "edit", keys: "Ctrl+A", title: () => (es() ? "Seleccionar todo" : "Select all"),
      when: reading, run: () => selectAll(activeDoc()!),
    }),
  ];
  return () => offs.forEach((off) => off());
}
