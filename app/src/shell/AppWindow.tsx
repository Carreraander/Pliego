import { createEffect, onMount, Show, type JSX } from "solid-js";
import { ToastRegion, type MenuEntry } from "../ui";
import { InspectorColumn, NoticeBar, Rail, SidePanel, StatusBar } from "./Regions";
import { ModeBar } from "./ModeBar";
import { TitleBar, type DocTab } from "./TitleBar";
import { isTauri, isWindows11 } from "./platform";
import { setHomeActive } from "./slots";
import { installShellShortcuts } from "./shortcuts";
import { st } from "./strings";
import "./shell.css";

export interface AppWindowProps {
  tabs: DocTab[];
  activeId: string | null;
  onSelectTab: (id: string) => void;
  onCloseTab: (id: string) => void;
  onNewTab: () => void;
  onReorderTabs: (from: number, to: number) => void;
  onDetachTab?: (id: string) => void;
  appMenu: MenuEntry[];
  /** El lienzo: visor, Inicio, Ajustes… */
  children: JSX.Element;
}

/**
 * Ventana completa de Pliego: barra de título, barra de modos, raíl, panel lateral, lienzo, inspector y barra de estado
 * (docs/design/ux-y-navegacion.md §2). Los demás flujos se montan con las ranuras de `slots.ts`; nadie edita este archivo.
 */
export function AppWindow(props: AppWindowProps) {
  installShellShortcuts();
  // Pestaña activa de tipo pantalla (Inicio, Ajustes): sin barra de modos ni estado de documento (§3.1 del diseño).
  const screen = () => props.tabs.find((t) => t.id === props.activeId)?.kind === "screen";
  createEffect(() => setHomeActive(screen()));
  onMount(async () => {
    // Mica solo en Windows 11 dentro de Tauri (`windowEffects: ["mica"]` en tauri.conf.json); si no, la barra queda opaca.
    if (isTauri() && (await isWindows11())) document.documentElement.setAttribute("data-mica", "");
  });
  return (
    <div class="pl-window" classList={{ "pl-window--home": screen() }}>
      <TitleBar tabs={props.tabs} activeId={props.activeId} onSelect={props.onSelectTab} onClose={props.onCloseTab} onNew={props.onNewTab}
        onReorder={props.onReorderTabs} onDetach={props.onDetachTab} appMenu={props.appMenu} />
      <Show when={!screen()}><ModeBar docId={props.activeId} /></Show>
      <div class="pl-body">
        <Rail docId={props.activeId} />
        <SidePanel docId={props.activeId} />
        <main class="pl-canvas" data-region="canvas" tabindex={-1} aria-label={st("shell.canvas")}>
          <NoticeBar docId={props.activeId} />
          <div class="pl-canvas__content">{props.children}</div>
        </main>
        <InspectorColumn />
      </div>
      <StatusBar />
      <ToastRegion />
    </div>
  );
}
