import { createSignal, For, Match, onMount, Show, Switch, type JSX } from "solid-js";
import { Icon } from "./Icon";
import { Kbd } from "./Display";
import { Popover } from "./Popover";
import type { Placement, Point } from "./overlay";
import "./Overlay.css";

export type MenuEntry =
  | { type?: "item"; id: string; label: string; icon?: string; shortcut?: string; disabled?: boolean; /** Motivo mostrado como ayuda si está desactivado. */ reason?: string;
      checked?: boolean; danger?: boolean; onSelect?: () => void; /** Submenú. */ items?: MenuEntry[] }
  | { type: "separator" }
  | { type: "label"; label: string };

const isItem = (e: MenuEntry): e is Extract<MenuEntry, { id: string }> => e.type === undefined || e.type === "item";

/** Lista de entradas de menú con teclado completo (flechas, Inicio/Fin, primera letra, → ← para submenús). */
function MenuList(props: { items: MenuEntry[]; label: string; close: () => void; closeSelf?: () => void }) {
  let list!: HTMLDivElement;
  const [openSub, setOpenSub] = createSignal<string | null>(null);
  const focusables = () => Array.from(list.querySelectorAll<HTMLElement>('[role^="menuitem"]:not([aria-disabled="true"])'));

  onMount(() => queueMicrotask(() => focusables()[0]?.focus()));

  const onKeyDown = (e: KeyboardEvent) => {
    const items = focusables();
    const i = items.indexOf(document.activeElement as HTMLElement);
    const go = (n: number) => { e.preventDefault(); items[(n + items.length) % items.length]?.focus(); };
    switch (e.key) {
      case "ArrowDown": return go(i + 1);
      case "ArrowUp": return go(i < 0 ? -1 : i - 1);
      case "Home": return go(0);
      case "End": return go(items.length - 1);
      case "ArrowLeft": if (props.closeSelf) { e.preventDefault(); e.stopPropagation(); props.closeSelf(); } return;
      case "Tab": e.preventDefault(); return props.close();
      default:
        if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
          const n = items.findIndex((it, k) => k > i && it.textContent?.trim().toLowerCase().startsWith(e.key.toLowerCase()));
          const m = n >= 0 ? n : items.findIndex((it) => it.textContent?.trim().toLowerCase().startsWith(e.key.toLowerCase()));
          if (m >= 0) go(m);
        }
    }
  };

  return (
    <div class="pl-menu" role="menu" aria-label={props.label} ref={list} onKeyDown={onKeyDown}>
      <For each={props.items}>{(entry) => (
        <Switch>
          <Match when={entry.type === "separator"}><hr class="pl-menu__sep" role="separator" /></Match>
          <Match when={entry.type === "label"}><div class="pl-menu__label pl-label-group" role="presentation">{(entry as { label: string }).label}</div></Match>
          <Match when={isItem(entry)}><MenuItem entry={entry as Extract<MenuEntry, { id: string }>} {...props} open={openSub()} setOpen={setOpenSub} /></Match>
        </Switch>
      )}</For>
    </div>
  );
}

function MenuItem(props: { entry: Extract<MenuEntry, { id: string }>; close: () => void; open: string | null; setOpen: (id: string | null) => void }) {
  let btn!: HTMLDivElement;
  const e = () => props.entry;
  const hasSub = () => !!e().items?.length;
  const role = () => (e().checked !== undefined ? "menuitemcheckbox" : "menuitem");
  const activate = () => {
    if (e().disabled) return;
    if (hasSub()) return props.setOpen(e().id);
    props.close();
    e().onSelect?.();
  };
  return (
    <>
      <div
        ref={btn} class="pl-menu__item" classList={{ "pl-menu__item--danger": e().danger }} role={role()} tabindex={-1}
        aria-disabled={e().disabled || undefined} aria-checked={e().checked} aria-haspopup={hasSub() ? "menu" : undefined}
        aria-expanded={hasSub() ? props.open === e().id : undefined} title={e().disabled ? e().reason : undefined}
        onClick={activate}
        onPointerEnter={() => { if (!e().disabled) { btn.focus(); props.setOpen(hasSub() ? e().id : null); } }}
        onKeyDown={(ev) => {
          if (ev.key === "Enter" || ev.key === " " || (ev.key === "ArrowRight" && hasSub())) { ev.preventDefault(); ev.stopPropagation(); activate(); }
        }}
      >
        <span class="pl-menu__icon">{e().checked ? <Icon name="check" size={16} /> : e().icon ? <Icon name={e().icon!} size={16} /> : null}</span>
        <span class="pl-menu__text">{e().label}</span>
        <Show when={e().shortcut}><Kbd keys={e().shortcut!} /></Show>
        <Show when={hasSub()}><Icon name="chevron-right" size={16} /></Show>
      </div>
      <Show when={hasSub()}>
        <Popover anchor={btn} open={props.open === e().id} placement="right-start" role="presentation" restoreFocus={false} onClose={() => props.setOpen(null)}>
          <MenuList items={e().items!} label={e().label} close={props.close} closeSelf={() => { props.setOpen(null); btn.focus(); }} />
        </Popover>
      </Show>
    </>
  );
}

/** Menú desplegable anclado a un elemento (botón de menú, logo de la app, «Herramientas ▾»). */
export function Menu(props: { anchor: HTMLElement | undefined; open: boolean; onClose: () => void; items: MenuEntry[]; label: string; placement?: Placement }) {
  return (
    <Popover anchor={props.anchor} open={props.open} onClose={props.onClose} placement={props.placement ?? "bottom-start"} role="presentation">
      <MenuList items={props.items} label={props.label} close={props.onClose} />
    </Popover>
  );
}

/** Menú contextual (clic derecho o tecla de menú) en un punto de la ventana. */
export function ContextMenu(props: { point: Point | null; onClose: () => void; items: MenuEntry[]; label: string }) {
  return (
    <Popover open={props.point !== null} point={props.point ?? undefined} onClose={props.onClose} role="presentation" restoreFocus={false}>
      <MenuList items={props.items} label={props.label} close={props.onClose} />
    </Popover>
  );
}

/** Ayuda para abrir un menú contextual desde un evento `contextmenu` (y `Mayús+F10`/tecla de menú). */
export function contextPoint(e: MouseEvent): Point {
  e.preventDefault();
  if (e.clientX === 0 && e.clientY === 0 && e.currentTarget instanceof HTMLElement) {
    const r = e.currentTarget.getBoundingClientRect();
    return { x: r.left + 16, y: r.top + 16 };
  }
  return { x: e.clientX, y: e.clientY };
}

/** Botón que abre un menú (envoltorio cómodo de `Menu`). */
export function MenuButton(props: { label: string; items: MenuEntry[]; children: (p: { toggle: () => void; open: boolean; ref: (el: HTMLElement) => void; }) => JSX.Element }) {
  const [open, setOpen] = createSignal(false);
  const [anchor, setAnchor] = createSignal<HTMLElement>();
  return (
    <>
      {props.children({ toggle: () => setOpen((o) => !o), open: open(), ref: setAnchor })}
      <Menu anchor={anchor()} open={open()} onClose={() => setOpen(false)} items={props.items} label={props.label} />
    </>
  );
}
