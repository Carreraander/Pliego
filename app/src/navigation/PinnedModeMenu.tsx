import { createSignal, onCleanup, onMount } from "solid-js";
import { ContextMenu, type Point } from "../ui";
import { modes } from "../shell";
import { unpinTool } from "./modes";
import { nt } from "./strings";

/**
 * Clic derecho sobre un modo fijado de la barra → «Quitar de la barra». La barra de modos es del armazón (nadie más la
 * edita), así que se escucha el evento en el documento y se reconoce la pestaña por su texto.
 */
export function PinnedModeMenu() {
  let last = ""; // el menú se cierra antes de llamar a onSelect
  const [target, setTarget] = createSignal<{ point: Point; toolId: string } | null>(null);
  onMount(() => {
    const onContext = (e: MouseEvent) => {
      const tab = (e.target as HTMLElement).closest<HTMLElement>('.pl-modebar [role="tab"], .pl-modebar [role="menuitem"]');
      if (!tab) return;
      const label = tab.textContent?.trim();
      const mode = modes().find((m) => m.id.startsWith("pin:") && m.title === label);
      if (!mode) return;
      e.preventDefault();
      last = mode.id.slice(4);
      setTarget({ point: { x: e.clientX, y: e.clientY }, toolId: mode.id.slice(4) });
    };
    document.addEventListener("contextmenu", onContext);
    onCleanup(() => document.removeEventListener("contextmenu", onContext));
  });
  return (
    <ContextMenu point={target()?.point ?? null} onClose={() => setTarget(null)} label={nt("nav.tools.unpin")}
      items={[{ id: "unpin", label: nt("nav.pinned.remove"), icon: "x", onSelect: () => { if (last) unpinTool(last); } }]} />
  );
}
