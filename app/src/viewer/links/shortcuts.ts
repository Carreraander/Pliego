import { locale } from "../../i18n";
import { registerShortcut } from "../../navigation/shortcuts";
import { currentScreen } from "../../navigation/screen";
import { activeDoc } from "../../state/documents";
import { canGoBack, canGoForward, goBack, goForward } from "./history";

/** Atrás y Adelante por la historia de saltos de los enlaces (Alt+← y Alt+→). */
export function registerJumpShortcuts(): () => void {
  const es = () => locale() === "es";
  const here = () => (currentScreen() === null ? activeDoc() : null);
  const offs = [
    registerShortcut({
      id: "nav.back", section: "navigation", keys: "Alt+ArrowLeft", title: () => (es() ? "Atrás en la historia de saltos" : "Back in jump history"),
      when: () => { const d = here(); return !!d && canGoBack(d.id); }, run: () => { const d = here(); if (d) goBack(d.id); },
    }),
    registerShortcut({
      id: "nav.forward", section: "navigation", keys: "Alt+ArrowRight", title: () => (es() ? "Adelante en la historia de saltos" : "Forward in jump history"),
      when: () => { const d = here(); return !!d && canGoForward(d.id); }, run: () => { const d = here(); if (d) goForward(d.id); },
    }),
  ];
  return () => offs.forEach((off) => off());
}
