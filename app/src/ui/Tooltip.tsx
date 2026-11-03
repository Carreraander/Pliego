import { createSignal, createUniqueId, onCleanup, Show, type JSX } from "solid-js";
import { Portal } from "solid-js/web";
import { computePosition, type Placement } from "./overlay";
import { Kbd } from "./Display";
import "./Overlay.css";

/**
 * Tooltip con atajo opcional. Aparece con retardo al pasar el ratón o al enfocar con teclado; `Esc` lo oculta.
 * Se enlaza al hijo con `aria-describedby`. Si el hijo ya tiene nombre accesible, el tooltip solo lo complementa.
 */
export function Tooltip(props: { text: string; shortcut?: string; placement?: Placement; delay?: number; children: JSX.Element }) {
  const id = createUniqueId();
  const [open, setOpen] = createSignal(false);
  const [pos, setPos] = createSignal({ x: 0, y: 0 });
  let host!: HTMLSpanElement;
  let tip: HTMLDivElement | undefined;
  let timer: number | undefined;

  const show = (delay: number) => {
    clearTimeout(timer);
    timer = window.setTimeout(() => {
      setOpen(true);
      queueMicrotask(() => {
        if (tip) setPos(computePosition(host.getBoundingClientRect(), { width: tip.offsetWidth, height: tip.offsetHeight }, props.placement ?? "bottom"));
      });
    }, delay);
  };
  const hide = () => { clearTimeout(timer); setOpen(false); };
  onCleanup(() => clearTimeout(timer));

  return (
    <span
      ref={host} class="pl-tooltip-host" aria-describedby={open() ? id : undefined}
      onPointerEnter={() => show(props.delay ?? 500)} onPointerLeave={hide} onPointerDown={hide}
      onFocusIn={(e) => { if (e.target instanceof HTMLElement && e.target.matches(":focus-visible")) show(150); }}
      onFocusOut={hide} onKeyDown={(e) => { if (e.key === "Escape") hide(); }}
    >
      {props.children}
      <Show when={open()}>
        <Portal>
          <div ref={tip} id={id} role="tooltip" class="pl-tooltip" style={{ left: `${pos().x}px`, top: `${pos().y}px` }}>
            {props.text}
            <Show when={props.shortcut}><Kbd keys={props.shortcut!} /></Show>
          </div>
        </Portal>
      </Show>
    </span>
  );
}
