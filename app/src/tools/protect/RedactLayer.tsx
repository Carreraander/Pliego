import { createSignal, For, Show } from "solid-js";
import { addArea, areasOnPage, marking, matchAreasOnPage, toArea } from "./redactState";
import "./redact.css";

/**
 * Capa que el visor monta dentro de cada página (coordenadas de la página sin girar, escala = zoom). Pinta las zonas
 * marcadas y las coincidencias de la vista previa y, con «Marcar zona» activo, recoge el arrastre.
 */
export function RedactLayer(props: { docId: number; page: number; zoom: number; size: { width: number; height: number } }) {
  const [start, setStart] = createSignal<{ x: number; y: number } | null>(null);
  const [now, setNow] = createSignal<{ x: number; y: number } | null>(null);
  const pt = (e: PointerEvent) => ({ x: e.offsetX / props.zoom, y: e.offsetY / props.zoom });
  const css = (a: { x0: number; y0: number; x1: number; y1: number }) => ({
    left: `${a.x0 * props.zoom}px`, top: `${a.y0 * props.zoom}px`, width: `${(a.x1 - a.x0) * props.zoom}px`, height: `${(a.y1 - a.y0) * props.zoom}px`,
  });
  const draft = () => {
    const a = start(), b = now();
    return a && b ? { x0: Math.min(a.x, b.x), y0: Math.min(a.y, b.y), x1: Math.max(a.x, b.x), y1: Math.max(a.y, b.y) } : null;
  };
  return (
    <div
      class="redact-layer" data-marking={marking()}
      style={{ "pointer-events": marking() ? "auto" : "none" }}
      onPointerDown={(e) => { if (!marking()) return; e.currentTarget.setPointerCapture(e.pointerId); setStart(pt(e)); setNow(pt(e)); }}
      onPointerMove={(e) => { if (start()) setNow(pt(e)); }}
      onPointerUp={(e) => {
        const a = start();
        if (a) { const area = toArea(props.page, a, pt(e), props.size); if (area) addArea(props.docId, area); }
        setStart(null); setNow(null);
      }}
      onPointerCancel={() => { setStart(null); setNow(null); }}
    >
      <For each={matchAreasOnPage(props.docId, props.page)}>{(a) => <div class="redact-match" style={css(a)} />}</For>
      <For each={areasOnPage(props.docId, props.page)}>{(a) => <div class="redact-mark" style={css(a)} />}</For>
      <Show when={draft()}>{(d) => <div class="redact-draft" style={css(d())} />}</Show>
    </div>
  );
}
