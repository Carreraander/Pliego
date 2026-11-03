import { For, Show } from "solid-js";
import { Dynamic } from "solid-js/web";
import { Button, Icon, MenuButton, Toolbar, type MenuEntry } from "../ui";
import { mode as layout } from "./layout";
import { activeMode, modes, openTools, setMode } from "./slots";
import { st } from "./strings";

/** Barra de modos de 44 px: modos a la izquierda, barra del modo activo a la derecha. < 1100 px los modos pasan a un desplegable. */
export function ModeBar(props: { docId: string | null }) {
  const compact = () => layout() !== "wide";
  const available = () => modes().filter((m) => !m.needsDocument || props.docId !== null);
  const entries = (): MenuEntry[] => available().map((m) => ({ id: m.id, label: m.title, icon: m.icon, checked: m.id === activeMode()?.id, onSelect: () => setMode(m.id) }));
  return (
    <div class="pl-modebar" data-region="modebar" data-compact={compact() || undefined} tabindex={-1}>
      <Show when={!compact()} fallback={
        <MenuButton label={st("shell.modes")} items={entries()}>
          {(m) => (
            <Button ref={m.ref} variant="ghost" icon={activeMode()?.icon} aria-haspopup="menu" aria-expanded={m.open} aria-label={st("shell.currentMode", { title: activeMode()?.title ?? "" })} onClick={m.toggle}>
              {activeMode()?.title}<Icon name="chevron-down" size={14} />
            </Button>
          )}
        </MenuButton>
      }>
        <Toolbar role="tablist" label={st("shell.modes")} class="pl-modebar__modes">
          <For each={available()}>{(m, i) => (
            <button type="button" role="tab" class="pl-modetab" aria-selected={m.id === activeMode()?.id} aria-keyshortcuts={i() < 9 ? `Alt+${i() + 1}` : undefined} onClick={() => setMode(m.id)}>{m.title}</button>
          )}</For>
        </Toolbar>
      </Show>
      <button type="button" class="pl-modetab pl-modetab--tools" aria-haspopup="dialog" aria-keyshortcuts="Control+Shift+A" onClick={() => openTools()}>
        {st("shell.tools")}<Show when={!compact()}><Icon name="chevron-down" size={14} /></Show>
      </button>
      <div class="pl-modebar__tools">
        <Show when={activeMode()?.toolbar} keyed>
          {(Tb) => <Toolbar label={st("shell.modeTools")}><Dynamic component={Tb} docId={props.docId} /></Toolbar>}
        </Show>
      </div>
    </div>
  );
}
