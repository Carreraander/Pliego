/**
 * Barra de modos: los seis modos por defecto (§4.2) y las herramientas que el usuario fija. Los modos de los demás flujos
 * se registran con `registerMode` (shell/slots) con el mismo id que su manifiesto y sustituyen al marcador de aquí.
 */
import { createEffect, createRoot } from "solid-js";
import { Button } from "../ui";
import { modes, registerMode, type ModeDef } from "../shell";
import { settings, updateSettings } from "../settings/store";
import { getTool, toolTitle, tools } from "../tools";
import { activateTool } from "./activate";
import { nt } from "./strings";

/** Modos fijos por defecto, en orden: [id del manifiesto, orden en la barra]. `read` lo registra la app (visor). */
export const DEFAULT_MODE_IDS = ["viewer.read", "annotate.comment", "forms.fill", "pages.organize", "edit.content", "protect.protect"] as const;

/** Un modo todavía sin implementar: barra con un aviso, para que los seis modos ya estén en su sitio. */
function placeholderMode(toolId: string, order: number): ModeDef {
  return {
    id: toolId, order, needsDocument: true, icon: getTool(toolId)?.icon,
    get title() { const t = getTool(toolId); return t ? toolTitle(t) : toolId; },
    toolbar: () => <span class="pl-muted pl-nav-soon">{nt("nav.mode.soon")}</span>,
  };
}

export function pinnedIds(): string[] { return settings().modes.pinned; }
export const isPinned = (id: string): boolean => pinnedIds().includes(id);

export function pinTool(id: string): void { if (!isPinned(id)) updateSettings("modes", { pinned: [...pinnedIds(), id] }); }
export function unpinTool(id: string): void { updateSettings("modes", { pinned: pinnedIds().filter((p) => p !== id) }); }
export function movePinned(id: string, dir: -1 | 1): void {
  const list = [...pinnedIds()];
  const i = list.indexOf(id);
  const j = i + dir;
  if (i < 0 || j < 0 || j >= list.length) return;
  [list[i], list[j]] = [list[j]!, list[i]!];
  updateSettings("modes", { pinned: list });
}

/** Registra los modos por defecto que aún no tiene nadie y mantiene en la barra las herramientas fijadas. */
export function installModes(): () => void {
  return createRoot((dispose) => {
    DEFAULT_MODE_IDS.forEach((id, i) => { if (id !== "viewer.read" && !modes().some((m) => m.id === id)) registerMode(placeholderMode(id, (i + 1) * 10)); });
    let offs: (() => void)[] = [];
    createEffect(() => {
      offs.forEach((off) => off());
      offs = [];
      pinnedIds().forEach((id, i) => {
        const tool = tools().find((t) => t.id === id);
        if (!tool || (DEFAULT_MODE_IDS as readonly string[]).includes(id)) return;
        const order = 100 + i;
        const existing = modes().find((m) => m.id === (tool.modeId ?? tool.id));
        if (tool.kind === "mode" && existing) return;
        offs.push(registerMode({
          id: `pin:${tool.id}`, title: toolTitle(tool), icon: tool.icon, order,
          toolbar: () => <Button variant="ghost" icon={tool.icon} onClick={() => void activateTool(tool)}>{nt("nav.mode.open", { title: toolTitle(tool) })}</Button>,
        }));
      });
    });
    return () => { offs.forEach((off) => off()); dispose(); };
  });
}

