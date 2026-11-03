import { createSignal } from "solid-js";
import "./shell.css";

/**
 * Divisor redimensionable (role="separator" con valor). Ratón: arrastrar; teclado: flechas (16 px, Mayús 64 px),
 * Inicio/Fin y doble clic para restablecer. `side` indica a qué lado del divisor está la columna que se redimensiona.
 */
export function Splitter(props: { label: string; value: number; min: number; max: number; side: "start" | "end"; onChange: (v: number) => void; onReset: () => void }) {
  const [dragging, setDragging] = createSignal(false);
  const dir = () => (props.side === "start" ? 1 : -1);
  return (
    <div
      class="pl-splitter" classList={{ "pl-splitter--drag": dragging() }} role="separator" aria-orientation="vertical" tabindex={0} aria-label={props.label}
      aria-valuenow={props.value} aria-valuemin={props.min} aria-valuemax={props.max}
      onPointerDown={(e) => {
        e.preventDefault();
        const startX = e.clientX, start = props.value;
        const el = e.currentTarget;
        el.setPointerCapture(e.pointerId);
        setDragging(true);
        const move = (ev: PointerEvent) => props.onChange(start + dir() * (ev.clientX - startX));
        const up = () => { setDragging(false); el.removeEventListener("pointermove", move); el.removeEventListener("pointerup", up); el.removeEventListener("pointercancel", up); };
        el.addEventListener("pointermove", move); el.addEventListener("pointerup", up); el.addEventListener("pointercancel", up);
      }}
      onDblClick={() => props.onReset()}
      onKeyDown={(e) => {
        const step = e.shiftKey ? 64 : 16;
        // «Más ancho» es →  para un panel a la izquierda y ← para uno a la derecha.
        const grow = props.side === "start" ? "ArrowRight" : "ArrowLeft";
        const shrink = props.side === "start" ? "ArrowLeft" : "ArrowRight";
        if (e.key === grow) props.onChange(props.value + step);
        else if (e.key === shrink) props.onChange(props.value - step);
        else if (e.key === "Home") props.onChange(props.min);
        else if (e.key === "End") props.onChange(props.max);
        else if (e.key === "Enter") props.onReset();
        else return;
        e.preventDefault();
      }}
    />
  );
}
