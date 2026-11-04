import { createEffect, createRoot, on } from "solid-js";
import { closeScreen, currentScreen, openScreen } from "../../navigation/screen";
import { activeMode, registerMode } from "../../shell";
import { activeId } from "../../state/documents";
import OrganizeGrid from "./OrganizeGrid";
import { OrganizeToolbar } from "./OrganizeToolbar";
import { ot } from "./text";

export const ORGANIZE_MODE = "pages.organize";
const SCREEN = "pages.organize.grid";

/** Registra el modo Organizar y mantiene la cuadrícula de miniaturas como pantalla mientras el modo está activo. */
export function installOrganizeMode(): () => void {
  const offMode = registerMode({
    id: ORGANIZE_MODE, order: 40, icon: "layout-grid", needsDocument: true,
    get title() { return ot("organize.mode.title"); },
    toolbar: OrganizeToolbar,
  });
  const dispose = createRoot((d) => {
    createEffect(on([() => activeMode()?.id, activeId], ([mode, doc]) => {
      if (mode === ORGANIZE_MODE && doc !== null) openScreen({ id: SCREEN, title: () => ot("organize.mode.title"), component: OrganizeGrid });
      else if (currentScreen()?.id === SCREEN) closeScreen();
    }));
    return d;
  });
  return () => { dispose(); offMode(); if (currentScreen()?.id === SCREEN) closeScreen(); };
}
