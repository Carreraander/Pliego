import { toast } from "../ui";
import { modes, openInspector, setMode } from "../shell";
import { setActivePanel, setSideOpen } from "../shell";
import { docFacts, toolState, toolTitle, type ToolManifest } from "../tools";
import { openScreen } from "./screen";

/** Lanza una herramienta según su tipo (§4.1). Si está desactivada, avisa del motivo. */
export async function activateTool(tool: ToolManifest): Promise<void> {
  const state = toolState(tool, docFacts());
  if (!state.enabled) { toast({ tone: "info", text: `${toolTitle(tool)}: ${state.reason}` }); return; }
  switch (tool.kind) {
    case "mode": {
      const id = tool.modeId ?? tool.id;
      if (modes().some((m) => m.id === id)) setMode(id);
      return;
    }
    case "panel":
      if (tool.panelId) { setActivePanel(tool.panelId); setSideOpen(true); }
      return;
    case "task":
    case "wizard": {
      const mod = await tool.load?.();
      if (mod) openInspector(mod.default, { toolId: tool.id });
      return;
    }
    case "screen": {
      const mod = await tool.load?.();
      if (mod) openScreen({ id: tool.id, title: () => toolTitle(tool), component: mod.default });
    }
  }
}

