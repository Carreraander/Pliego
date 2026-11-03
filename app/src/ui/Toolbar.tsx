import { onMount, type JSX } from "solid-js";
import "./Toolbar.css";

const FOCUSABLE = 'button:not([disabled]), [role="tab"]:not([disabled]), [role="radio"], input:not([disabled]), select:not([disabled])';

/**
 * Barra de herramientas WAI-ARIA: una sola parada de tabulación y flechas para moverse entre controles
 * (el último foco se recuerda). Sirve para la barra del modo y para el raíl (`orientation="vertical"`).
 */
export function Toolbar(props: { label: string; orientation?: "horizontal" | "vertical"; class?: string; children: JSX.Element; role?: "toolbar" | "tablist" }) {
  let el!: HTMLDivElement;
  const items = () => Array.from(el.querySelectorAll<HTMLElement>(FOCUSABLE));
  const rove = (active: HTMLElement | undefined) => items().forEach((i) => i.setAttribute("tabindex", i === active ? "0" : "-1"));
  onMount(() => {
    const observer = new MutationObserver(() => { if (!items().some((i) => i.tabIndex === 0)) rove(items()[0]); });
    observer.observe(el, { childList: true, subtree: true });
    rove(items().find((i) => i.getAttribute("aria-selected") === "true" || i.getAttribute("aria-pressed") === "true") ?? items()[0]);
    return () => observer.disconnect();
  });
  const vertical = () => props.orientation === "vertical";
  return (
    <div
      ref={el} class={`pl-toolbar ${vertical() ? "pl-toolbar--v" : ""} ${props.class ?? ""}`} role={props.role ?? "toolbar"} aria-label={props.label}
      aria-orientation={props.orientation ?? "horizontal"}
      onFocusIn={(e) => { if ((e.target as HTMLElement).matches(FOCUSABLE)) rove(e.target as HTMLElement); }}
      onKeyDown={(e) => {
        const prev = vertical() ? "ArrowUp" : "ArrowLeft";
        const nextKey = vertical() ? "ArrowDown" : "ArrowRight";
        if (e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement) return;
        const list = items();
        const i = list.indexOf(e.target as HTMLElement);
        const to = e.key === nextKey ? i + 1 : e.key === prev ? i - 1 : e.key === "Home" ? 0 : e.key === "End" ? list.length - 1 : null;
        if (to === null || i < 0) return;
        e.preventDefault();
        list[(to + list.length) % list.length]?.focus();
      }}
    >{props.children}</div>
  );
}

/** Separador vertical dentro de una barra de herramientas. */
export function ToolbarSeparator() { return <span class="pl-toolbar__sep" role="separator" aria-orientation="vertical" />; }
