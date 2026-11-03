import { createEffect, createSignal, createUniqueId, on, onCleanup, Show, type JSX } from "solid-js";
import { Portal } from "solid-js/web";
import { computePointPosition, computePosition, type Placement, type Point } from "./overlay";
import "./Overlay.css";

/**
 * Superposición anclada a un elemento. Se cierra con `Esc` y con clic fuera; al cerrarse devuelve el foco al ancla.
 * Es la base de `Menu`, la paleta de herramientas y el centro de tareas.
 */
export function Popover(props: {
  anchor?: HTMLElement | undefined;
  /** Alternativa al ancla: un punto de la ventana (menú contextual). */
  point?: Point;
  /** Devuelve el foco al ancla al cerrar con `Esc` (por defecto sí). */
  restoreFocus?: boolean;
  open: boolean;
  onClose: () => void;
  placement?: Placement;
  /** Nombre accesible del diálogo (si es un panel); los menús usan su propio rol. */
  label?: string;
  role?: "dialog" | "menu" | "listbox" | "presentation";
  class?: string;
  children: JSX.Element;
  ref?: (el: HTMLDivElement) => void;
}) {
  let el: HTMLDivElement | undefined;
  const [pos, setPos] = createSignal({ x: 0, y: 0 });
  const id = createUniqueId();

  const place = () => {
    if (!el) return;
    const size = { width: el.offsetWidth, height: el.offsetHeight };
    if (props.point) setPos(computePointPosition(props.point, size));
    else if (props.anchor) setPos(computePosition(props.anchor.getBoundingClientRect(), size, props.placement ?? "bottom-start"));
  };

  createEffect(on(() => props.open, (open) => {
    if (!open) return;
    queueMicrotask(place);
    const onDown = (e: PointerEvent) => {
      const t = e.target as Node;
      if (el?.contains(t) || props.anchor?.contains(t)) return;
      props.onClose();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") { e.stopPropagation(); props.onClose(); if (props.restoreFocus !== false) props.anchor?.focus(); }
    };
    document.addEventListener("pointerdown", onDown, true);
    document.addEventListener("keydown", onKey, true);
    addEventListener("resize", place);
    onCleanup(() => {
      document.removeEventListener("pointerdown", onDown, true);
      document.removeEventListener("keydown", onKey, true);
      removeEventListener("resize", place);
    });
  }));

  return (
    <Show when={props.open}>
      <Portal>
        <div
          ref={(n) => { el = n; props.ref?.(n); }} id={id} class={`pl-popover ${props.class ?? ""}`} role={props.role ?? "dialog"}
          aria-label={props.label} style={{ left: `${pos().x}px`, top: `${pos().y}px` }}
        >{props.children}</div>
      </Portal>
    </Show>
  );
}
