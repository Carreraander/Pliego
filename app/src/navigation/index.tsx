import { createEffect, createRoot } from "solid-js";
import { locale } from "../i18n";
import { setPaletteOpener, setToolsOpener } from "../shell";
import { registerBaseTools, registerToolIcons, tools, toolTitle } from "../tools";
import { activateTool } from "./activate";
import { CommandPalette, openCommandPalette } from "./CommandPalette";
import { installCommandProviders } from "./commands";
import { installModes } from "./modes";
import { PinnedModeMenu } from "./PinnedModeMenu";
import { registerReferenceShortcuts } from "./reference";
import { openShortcutSheet, ShortcutSheet } from "./ShortcutSheet";
import { installShortcuts, registerShortcut } from "./shortcuts";
import { nt } from "./strings";
import { ToolsPalette, toggleToolsPalette } from "./ToolsPalette";
import "./navigation.css";

export { activateTool } from "./activate";
export { openCommandPalette } from "./CommandPalette";
export { openToolsPalette } from "./ToolsPalette";
export { openShortcutSheet } from "./ShortcutSheet";
export { registerCommand, registerCommandProvider, type Command, type CommandProvider } from "./commands";
export { registerShortcut, type ShortcutDef } from "./shortcuts";
export { currentScreen, openScreen, closeScreen } from "./screen";
export { pinTool, unpinTool, isPinned } from "./modes";

/** Monta paletas y hoja de atajos. Va una sola vez dentro de la ventana (App.tsx). */
export function NavigationHost() {
  return <><CommandPalette /><ToolsPalette /><ShortcutSheet /><PinnedModeMenu /></>;
}

/** Arranca la navegación: iconos, herramientas de base, proveedores de la paleta, modos, atajos y los enganches del armazón. */
export function installNavigation(): () => void {
  registerToolIcons();
  registerBaseTools();
  setPaletteOpener((q) => openCommandPalette(q ?? ""));
  setToolsOpener(toggleToolsPalette);
  const offs = [
    installCommandProviders(),
    installModes(),
    registerShortcut({ id: "help.shortcuts", section: "panels", keys: "F1", title: () => nt("nav.cmd.shortcuts"), run: openShortcutSheet }),
    registerShortcut({ id: "nav.goto", section: "navigation", keys: "Ctrl+G", title: () => (locale() === "es" ? "Ir a la página" : "Go to page"), run: () => openCommandPalette("") }),
    registerReferenceShortcuts(new Set(["nav.goto"])),
    installShortcuts(),
  ];
  // Atajos de las herramientas reales (los de las fantasma no hacen nada hasta que existan).
  const dispose = createRoot((d) => {
    let toolOffs: (() => void)[] = [];
    createEffect(() => {
      toolOffs.forEach((off) => off());
      toolOffs = tools().filter((t) => t.shortcut && !t.ghost).map((t) => registerShortcut({
        id: `tool:${t.id}`, section: t.shortcut!.includes("+") ? "file" : "tools", keys: t.shortcut!, scope: t.shortcut!.includes("+") ? "global" : "canvas",
        title: () => toolTitle(t), run: () => void activateTool(t),
      }));
    });
    return () => { toolOffs.forEach((off) => off()); d(); };
  });
  return () => { offs.forEach((off) => off()); dispose(); setPaletteOpener(null); setToolsOpener(null); };
}
