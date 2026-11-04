import { For, Show } from "solid-js";
import { Dynamic } from "solid-js/web";
import { IconButton, Notice, Tooltip } from "../ui";
import {
  activePanel, INSPECTOR, inspectorWidth, mode as layout, SIDEPANEL, setInspectorWidth, setSidePanelWidth, sideOpen, setSideOpen, sidePanelWidth, toggleRail,
} from "./layout";
import { Splitter } from "./Splitter";
import { closeInspector, dismissNotice, inspector, noticeFor, panels, statusItems, type StatusItem } from "./slots";
import { st } from "./strings";

/** Raíl de 48 px con los paneles registrados. Clic en el activo pliega el panel. */
export function Rail(props: { docId: string | null }) {
  const visible = () => panels().filter((p) => !(p.needsDocument ?? true) || props.docId !== null);
  return (
    <nav class="pl-rail" data-region="rail" aria-label={st("shell.rail")}>
      <div class="pl-rail__list" role="toolbar" aria-orientation="vertical" aria-label={st("shell.rail")} onKeyDown={(e) => railKeys(e)}>
        <For each={visible()}>{(p) => (
          <Tooltip text={p.title} shortcut={p.shortcut} placement="right-start">
            <IconButton icon={p.icon} label={p.title} pressed={sideOpen() && activePanel() === p.id} aria-controls="pl-sidepanel" onClick={() => toggleRail(p.id)} />
          </Tooltip>
        )}</For>
      </div>
    </nav>
  );
}
function railKeys(e: KeyboardEvent) {
  const items = Array.from((e.currentTarget as HTMLElement).querySelectorAll<HTMLElement>("button"));
  const i = items.indexOf(e.target as HTMLElement);
  const to = e.key === "ArrowDown" ? i + 1 : e.key === "ArrowUp" ? i - 1 : e.key === "Home" ? 0 : e.key === "End" ? items.length - 1 : null;
  if (to === null || i < 0) return;
  e.preventDefault();
  items[(to + items.length) % items.length]?.focus();
}

/** Panel lateral redimensionable (200–480 px, ancho recordado por panel). */
export function SidePanel(props: { docId: string | null }) {
  const panel = () => panels().find((p) => p.id === activePanel()) ?? null;
  const open = () => sideOpen() && panel() !== null && ((panel()!.needsDocument ?? true) ? props.docId !== null : true);
  return (
    <Show when={open() ? panel() : null} keyed>
      {(p) => (
        <>
          <aside id="pl-sidepanel" class="pl-sidepanel" data-region="sidepanel" tabindex={-1} aria-label={p.title} classList={{ "pl-overlay-left": layout() === "narrow" }} style={{ width: `${sidePanelWidth(p.id)}px` }}>
            <header class="pl-sidepanel__head">
              <h2>{p.title}</h2>
              <IconButton icon="x" size="sm" label={st("shell.closeSide")} onClick={() => setSideOpen(false)} />
            </header>
            <div class="pl-sidepanel__body"><Dynamic component={p.component} docId={props.docId} /></div>
          </aside>
          <Show when={layout() !== "narrow"}>
            <Splitter label={st("shell.resizeSide")} side="start" value={sidePanelWidth(p.id)} min={SIDEPANEL.min} max={SIDEPANEL.max}
              onChange={(v) => setSidePanelWidth(p.id, v)} onReset={() => setSidePanelWidth(p.id, SIDEPANEL.default)} />
          </Show>
        </>
      )}
    </Show>
  );
}

/** Columna del inspector (280–480 px). Se abre sola cuando algún flujo llama a `openInspector`. */
export function InspectorColumn() {
  return (
    <Show when={inspector()} keyed>
      {(c) => (
        <>
          <Show when={layout() !== "narrow"}>
            <Splitter label={st("shell.resizeInspector")} side="end" value={inspectorWidth()} min={INSPECTOR.min} max={INSPECTOR.max}
              onChange={setInspectorWidth} onReset={() => setInspectorWidth(INSPECTOR.default)} />
          </Show>
          <aside class="pl-inspector-col" data-region="inspector" tabindex={-1} aria-label={st("shell.inspector")} classList={{ "pl-overlay-right": layout() === "narrow" }} style={{ width: `${inspectorWidth()}px` }}>
            <Dynamic component={c.component} onClose={closeInspector} {...c.props} />
          </aside>
        </>
      )}
    </Show>
  );
}

/** Barra de aviso del documento pegada arriba del lienzo (una sola, por prioridad). */
export function NoticeBar(props: { docId: string | null }) {
  const n = () => noticeFor(props.docId);
  return (
    <Show when={n()} keyed>
      {(notice) => (
        <Notice tone={notice.tone} actions={notice.actions} onDismiss={notice.dismissible ? () => { dismissNotice(props.docId!, notice.id); notice.onDismiss?.(); } : undefined}>{notice.text}</Notice>
      )}
    </Show>
  );
}

/** Barra de estado de 28 px con tres ranuras. */
export function StatusBar() {
  const slot = (s: StatusItem["slot"]) => (
    <div class={`pl-status__slot pl-status__slot--${s}`}>
      <For each={statusItems(s)}>{(i) => <StatusEntry item={i} />}</For>
    </div>
  );
  return (
    <footer class="pl-status" data-region="statusbar" tabindex={-1} aria-label={st("shell.status")}>
      {slot("start")}{slot("center")}{slot("end")}
    </footer>
  );
}

function StatusEntry(props: { item: StatusItem }) {
  const content = () => (props.item.component ? <Dynamic component={props.item.component} /> : <span class="pl-num">{props.item.text?.()}</span>);
  return (
    <Show when={props.item.onClick} fallback={<span class={`pl-status__item ${props.item.tone ? `pl-status__item--${props.item.tone}` : ""}`} title={props.item.label}>{content()}</span>}>
      <button type="button" class={`pl-status__item pl-status__item--btn ${props.item.tone ? `pl-status__item--${props.item.tone}` : ""}`} title={props.item.label} onClick={() => props.item.onClick?.()}>{content()}</button>
    </Show>
  );
}

