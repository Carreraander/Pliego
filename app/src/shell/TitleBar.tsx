import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { Dynamic } from "solid-js/web";
import { Icon, IconButton, Kbd, Logo, Menu, Tooltip, type MenuEntry } from "../ui";
import { openPalette, titleItems } from "./slots";
import { isTauri, tauriWindow } from "./platform";
import { st } from "./strings";

export interface DocTab {
  id: string;
  title: string;
  dirty?: boolean;
  /** `document` (por defecto) o `screen` (Inicio, Ajustes): una pantalla oculta la barra de modos y el estado de documento. */
  kind?: "document" | "screen";
}

export interface TitleBarProps {
  tabs: DocTab[];
  activeId: string | null;
  onSelect: (id: string) => void;
  onClose: (id: string) => void;
  onNew: () => void;
  /** Mueve la pestaña de `from` a `to` (índices). */
  onReorder: (from: number, to: number) => void;
  /** Arrastrar una pestaña fuera de la ventana: abrir ventana nueva (opcional). */
  onDetach?: (id: string) => void;
  appMenu: MenuEntry[];
}

/** Barra de título de 40 px: menú de la app, pestañas de documento, buscador/paleta, huecos y botones de ventana. */
export function TitleBar(props: TitleBarProps) {
  const [menuOpen, setMenuOpen] = createSignal(false);
  const [menuAnchor, setMenuAnchor] = createSignal<HTMLElement>();
  return (
    <header class="pl-titlebar" data-region="titlebar" data-tauri-drag-region tabindex={-1} aria-label={st("shell.titlebar")}>
      <button
        ref={setMenuAnchor} type="button" class="pl-titlebar__logo" aria-label={st("shell.appMenu")} aria-haspopup="menu" aria-expanded={menuOpen()}
        onClick={() => setMenuOpen(!menuOpen())}
      >
        <Logo size={18} /><Icon name="chevron-down" size={14} />
      </button>
      <Menu anchor={menuAnchor()} open={menuOpen()} onClose={() => setMenuOpen(false)} items={props.appMenu} label={st("shell.appMenu")} />
      <DocTabs {...props} />
      <Tooltip text={st("shell.newTab")} shortcut="Ctrl+T"><IconButton icon="plus" size="sm" label={st("shell.newTab")} onClick={props.onNew} /></Tooltip>
      <div class="pl-titlebar__drag" data-tauri-drag-region />
      <button type="button" class="pl-titlebar__search" onClick={() => openPalette()} aria-keyshortcuts="Control+K">
        <Icon name="search" size={16} /><span>{st("shell.search")}</span><Kbd keys="Ctrl+K" />
      </button>
      <For each={titleItems()}>{(item) => <div class="pl-titlebar__item"><Dynamic component={item.component} /></div>}</For>
      <WindowButtons />
    </header>
  );
}

/** Pestañas arrastrables y reordenables. Teclado: flechas para moverse, `Ctrl+Mayús+←/→` reordenar, `Supr` cerrar. */
function DocTabs(props: TitleBarProps) {
  let list!: HTMLDivElement;
  const [drag, setDrag] = createSignal<{ id: string; dx: number; to: number } | null>(null);

  const onPointerDown = (e: PointerEvent, index: number, id: string) => {
    if (e.button !== 0) return;
    const startX = e.clientX, startY = e.clientY;
    const els = Array.from(list.querySelectorAll<HTMLElement>(".pl-doctab"));
    const rects = els.map((el) => el.getBoundingClientRect());
    let active = false;
    const move = (ev: PointerEvent) => {
      const dx = ev.clientX - startX;
      if (!active && Math.abs(dx) < 5) return;
      active = true;
      const center = rects[index]!.left + rects[index]!.width / 2 + dx;
      let to = rects.findIndex((r) => center < r.left + r.width / 2);
      if (to < 0) to = rects.length - 1;
      setDrag({ id, dx, to });
    };
    const up = (ev: PointerEvent) => {
      removeEventListener("pointermove", move); removeEventListener("pointerup", up);
      const d = drag();
      setDrag(null);
      if (!active) return;
      if (ev.clientY > startY + 80 || ev.clientY < startY - 80) props.onDetach?.(id);
      else if (d && d.to !== index) props.onReorder(index, d.to);
    };
    addEventListener("pointermove", move); addEventListener("pointerup", up);
  };

  const onKeyDown = (e: KeyboardEvent, index: number, id: string) => {
    if (e.ctrlKey && e.shiftKey && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
      e.preventDefault();
      const to = index + (e.key === "ArrowRight" ? 1 : -1);
      if (to >= 0 && to < props.tabs.length) { props.onReorder(index, to); queueMicrotask(() => list.querySelector<HTMLElement>(`[role=tab][data-id="${id}"]`)?.focus()); }
      return;
    }
    if (e.key === "Delete") { e.preventDefault(); props.onClose(id); return; }
    const to = e.key === "ArrowRight" ? index + 1 : e.key === "ArrowLeft" ? index - 1 : e.key === "Home" ? 0 : e.key === "End" ? props.tabs.length - 1 : null;
    if (to === null) return;
    e.preventDefault();
    const target = props.tabs[(to + props.tabs.length) % props.tabs.length];
    if (target) { props.onSelect(target.id); queueMicrotask(() => list.querySelector<HTMLElement>(`[role=tab][data-id="${target.id}"]`)?.focus()); }
  };

  onMount(() => { /* el contenedor se mide al arrastrar */ });
  onCleanup(() => setDrag(null));

  return (
    <div ref={list} class="pl-doctabs" role="tablist" aria-label={st("shell.documents")}>
      <For each={props.tabs}>{(tab, i) => (
        <div
          class="pl-doctab" role="presentation" classList={{ "pl-doctab--active": tab.id === props.activeId, "pl-doctab--dragging": drag()?.id === tab.id }}
          style={drag()?.id === tab.id ? { transform: `translateX(${drag()!.dx}px)` } : undefined}
          onAuxClick={(e) => { if (e.button === 1) props.onClose(tab.id); }}
        >
          <button
            type="button" role="tab" data-id={tab.id} class="pl-doctab__btn" aria-selected={tab.id === props.activeId} tabindex={tab.id === props.activeId ? 0 : -1}
            title={tab.title} onClick={() => props.onSelect(tab.id)} onPointerDown={(e) => onPointerDown(e, i(), tab.id)} onKeyDown={(e) => onKeyDown(e, i(), tab.id)}
          >
            <span class="pl-doctab__title">{tab.title}</span>
            <Show when={tab.dirty}><span class="pl-sr-only"> ({st("shell.unsaved")})</span></Show>
          </button>
          {/* Cerrar: con ratón; con teclado, Supr o Ctrl+W (un botón dentro de un tablist rompería ARIA) */}
          <button type="button" class="pl-doctab__close" aria-hidden="true" tabindex={-1} classList={{ "pl-doctab__close--dirty": tab.dirty }} onClick={() => props.onClose(tab.id)}>
            <Show when={tab.dirty} fallback={<Icon name="x" size={14} />}><span class="pl-doctab__dot" /><span class="pl-doctab__x"><Icon name="x" size={14} /></span></Show>
          </button>
        </div>
      )}</For>
    </div>
  );
}

/** Minimizar, maximizar/restaurar y cerrar (solo dentro de Tauri; en el navegador no se muestran). */
function WindowButtons() {
  const [maximized, setMaximized] = createSignal(false);
  onMount(async () => {
    const w = await tauriWindow();
    if (!w) return;
    setMaximized(await w.isMaximized());
    const off = await w.onResized(async () => setMaximized(await w.isMaximized()));
    onCleanup(off);
  });
  const act = async (a: "minimize" | "toggleMaximize" | "close") => { const w = await tauriWindow(); await w?.[a](); };
  return (
    <Show when={isTauri()}>
      <div class="pl-winbtns">
        <button type="button" class="pl-winbtn" aria-label={st("shell.minimize")} onClick={() => void act("minimize")}><Icon name="minus" size={16} /></button>
        <button type="button" class="pl-winbtn" aria-label={maximized() ? st("shell.restore") : st("shell.maximize")} onClick={() => void act("toggleMaximize")}>
          <Icon name={maximized() ? "minimize" : "maximize"} size={14} />
        </button>
        <button type="button" class="pl-winbtn pl-winbtn--close" aria-label={st("shell.close")} onClick={() => void act("close")}><Icon name="x" size={16} /></button>
      </div>
    </Show>
  );
}
