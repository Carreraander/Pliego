import { createContext, createUniqueId, For, Show, useContext, type JSX } from "solid-js";
import { Icon } from "./Icon";
import "./Tabs.css";

export interface TabDef { id: string; label: string; icon?: string; disabled?: boolean }

interface Ctx { base: string; value: () => string }
const TabsContext = createContext<Ctx>();

/**
 * Pestañas WAI-ARIA (tablist + tabpanel). Flechas, Inicio y Fin mueven la selección. Los paneles se declaran con
 * `<TabPanel id="…">` dentro de `Tabs`. No confundir con las pestañas de documento de la barra de título (`shell/`).
 */
export function Tabs(props: { label: string; tabs: TabDef[]; value: string; onChange: (id: string) => void; variant?: "line" | "pill"; children?: JSX.Element; class?: string }) {
  const base = createUniqueId();
  const onKeyDown = (e: KeyboardEvent) => {
    const els = Array.from((e.currentTarget as HTMLElement).querySelectorAll<HTMLElement>('[role="tab"]:not([disabled])'));
    const i = els.indexOf(document.activeElement as HTMLElement);
    const to = e.key === "ArrowRight" ? i + 1 : e.key === "ArrowLeft" ? i - 1 : e.key === "Home" ? 0 : e.key === "End" ? els.length - 1 : null;
    if (to === null) return;
    e.preventDefault();
    const el = els[(to + els.length) % els.length]!;
    el.focus(); el.click();
  };
  return (
    <TabsContext.Provider value={{ base, value: () => props.value }}>
      <div class={`pl-tabs ${props.class ?? ""}`}>
        <div class={`pl-tabs__list pl-tabs__list--${props.variant ?? "line"}`} role="tablist" aria-label={props.label} onKeyDown={onKeyDown}>
          <For each={props.tabs}>{(t) => (
            <button type="button" role="tab" class="pl-tabs__tab" id={`${base}-tab-${t.id}`} aria-controls={`${base}-panel-${t.id}`}
              aria-selected={t.id === props.value} tabindex={t.id === props.value ? 0 : -1} disabled={t.disabled} onClick={() => props.onChange(t.id)}>
              <Show when={t.icon}><Icon name={t.icon!} size={16} /></Show>{t.label}
            </button>
          )}</For>
        </div>
        {props.children}
      </div>
    </TabsContext.Provider>
  );
}

export function TabPanel(props: { id: string; children: JSX.Element }) {
  const c = useContext(TabsContext)!;
  return (
    <Show when={c.value() === props.id}>
      <div class="pl-tabs__panel" role="tabpanel" id={`${c.base}-panel-${props.id}`} aria-labelledby={`${c.base}-tab-${props.id}`} tabindex={0}>{props.children}</div>
    </Show>
  );
}
