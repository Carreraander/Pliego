import { createEffect, createMemo, createSignal, For, on, Show } from "solid-js";
import { locale } from "../i18n";
import { ContextMenu, contextPoint, Icon, Kbd, type MenuEntry, type Point } from "../ui";
import { docFacts, toolDescription, toolState, toolsByGroup, toolTitle, type ToolManifest } from "../tools";
import { toolText } from "../tools/strings";
import { activateTool } from "./activate";
import { bestScore } from "./fuzzy";
import { displayCombo } from "./keys";
import { isPinned, movePinned, pinnedIds, pinTool, unpinTool } from "./modes";
import { nt } from "./strings";

const [open, setOpen] = createSignal(false);
export function openToolsPalette(): void { setOpen(true); }
/** `Ctrl+Mayús+A` la abre y la cierra. */
export function toggleToolsPalette(): void { setOpen((o) => !o); }
export const closeToolsPalette = (): void => { setOpen(false); };

/** Paleta «Herramientas» (`Ctrl+Mayús+A`): todas las herramientas por grupo (§4.3), con buscador; las desactivadas dicen por qué. */
export function ToolsPalette() {
  let dialog!: HTMLDialogElement;
  let input!: HTMLInputElement;
  const [q, setQ] = createSignal("");
  const [menu, setMenu] = createSignal<{ point: Point; tool: ToolManifest } | null>(null);

  let opener: HTMLElement | null = null;
  const onPointerDown = (e: PointerEvent) => {
    const target = e.target as HTMLElement;
    if (!dialog.contains(target) && !target.closest(".pl-popover, .pl-modetab--tools")) setOpen(false);
  };
  const onEscape = (e: KeyboardEvent) => {
    if (e.key === "Escape" && !e.defaultPrevented && !document.querySelector(".pl-popover")) { e.preventDefault(); setOpen(false); }
  };
  createEffect(on(open, (isOpen) => {
    if (isOpen && !dialog.open) {
      opener = document.activeElement as HTMLElement | null;
      setQ(""); dialog.show(); queueMicrotask(() => input.focus());
      addEventListener("pointerdown", onPointerDown, true);
      addEventListener("keydown", onEscape);
    } else if (!isOpen && dialog.open) {
      dialog.close();
      removeEventListener("pointerdown", onPointerDown, true);
      removeEventListener("keydown", onEscape);
      if (opener?.isConnected) opener.focus();
      opener = null;
    }
  }));

  const groups = createMemo(() => toolsByGroup().map((g) => ({
    ...g,
    tools: g.tools.filter((t) => !q().trim() || bestScore(q(), [toolTitle(t), ...(t.keywords ?? [])], [toolDescription(t)]) !== null),
  })).filter((g) => g.tools.length > 0));

  const choose = (tool: ToolManifest) => {
    if (!toolState(tool, docFacts()).enabled) return;
    setOpen(false);
    queueMicrotask(() => void activateTool(tool));
  };
  const menuItems = (tool: ToolManifest): MenuEntry[] => {
    const entries: MenuEntry[] = [{ id: "open", label: nt("nav.tools.open", { title: toolTitle(tool) }), icon: tool.icon, disabled: !toolState(tool, docFacts()).enabled, onSelect: () => choose(tool) }];
    if (tool.pinnable) {
      entries.push({ type: "separator" });
      entries.push(isPinned(tool.id)
        ? { id: "unpin", label: nt("nav.tools.unpin"), icon: "x", onSelect: () => unpinTool(tool.id) }
        : { id: "pin", label: nt("nav.tools.pin"), icon: "star", onSelect: () => pinTool(tool.id) });
      if (isPinned(tool.id)) {
        const i = pinnedIds().indexOf(tool.id);
        entries.push({ id: "left", label: nt("nav.tools.moveLeft"), icon: "arrow-left", disabled: i <= 0, onSelect: () => movePinned(tool.id, -1) });
        entries.push({ id: "right", label: nt("nav.tools.moveRight"), icon: "arrow-right", disabled: i >= pinnedIds().length - 1, onSelect: () => movePinned(tool.id, 1) });
      }
    }
    return entries;
  };

  return (
    <dialog ref={dialog} class="pl-palette pl-palette--tools" role="dialog" aria-label={nt("nav.tools")}
      onFocusOut={(e) => { const next = e.relatedTarget as HTMLElement | null; if (next && !dialog.contains(next) && !next.closest(".pl-popover")) setOpen(false); }} onCancel={(e) => { e.preventDefault(); setOpen(false); }} onClose={() => setOpen(false)}
      >
      <div class="pl-palette__search">
        <Icon name="search" size={18} />
        <input ref={input} type="search" aria-label={nt("nav.tools.search")} placeholder={nt("nav.tools.search")} autocomplete="off" value={q()} onInput={(e) => setQ(e.currentTarget.value)} />
        <Kbd keys="Esc" />
      </div>
      <div class="pl-toolgroups">
        <For each={groups()} fallback={<div class="pl-palette__empty" role="status">{nt("nav.tools.none", { q: q() })}</div>}>{(g) => (
          <section aria-label={toolText(`group.${g.group}`, locale())}>
            <h3 class="pl-label-group">{toolText(`group.${g.group}`, locale())}</h3>
            <div class="pl-toolgroup__grid">
              <For each={g.tools}>{(tool) => {
                const state = () => toolState(tool, docFacts());
                const reason = () => { const s = state(); return s.enabled ? undefined : s.reason; };
                return (
                  <button type="button" class="pl-tool" data-tool={tool.id} aria-disabled={state().enabled ? undefined : true} title={reason()}
                    aria-describedby={undefined} onClick={() => choose(tool)} onContextMenu={(e) => { if (tool.pinnable) setMenu({ point: contextPoint(e), tool }); }}>
                    <Icon name={tool.icon} size={20} />
                    <span>
                      <span class="pl-tool__title">{toolTitle(tool)}</span>
                      <span class="pl-tool__desc">{reason() ?? toolDescription(tool)}</span>
                    </span>
                    <Show when={isPinned(tool.id)}><span class="pl-tool__pin" title={nt("nav.pinned")}><Icon name="star" size={14} /></span></Show>
                    <Show when={tool.shortcut && !isPinned(tool.id)}><Kbd keys={displayCombo(tool.shortcut!, locale())} /></Show>
                  </button>
                );
              }}</For>
            </div>
          </section>
        )}</For>
      </div>
      <Show when={menu()} keyed>{(m) => <ContextMenu point={m.point} onClose={() => setMenu(null)} items={menuItems(m.tool)} label={toolTitle(m.tool)} />}</Show>
    </dialog>
  );
}
