import { createSignal } from "solid-js";
import { Button } from "../../ui";
import type { Pt } from "../annotate/geometry";
import { normalize, saveSignature } from "./signature";
import { setPlacingSignature, setSignature, setSignaturePadOpen } from "./state";
import { ft } from "./text";

/** Zona para dibujar «Mi firma». Al guardar, la firma queda en este equipo y pasa a colocarse en la página. */
export function SignaturePad() {
  const [strokes, setStrokes] = createSignal<Pt[][]>([]);
  const [drawing, setDrawing] = createSignal(false);
  const pt = (e: PointerEvent): Pt => [e.offsetX, e.offsetY];
  const path = () => strokes().map((s) => s.map((q, i) => `${i ? "L" : "M"}${q[0]} ${q[1]}`).join(" ")).join(" ");
  const save = () => {
    const sig = normalize(strokes());
    if (!sig) return;
    saveSignature(sig);
    setSignature(sig);
    setSignaturePadOpen(false);
    setPlacingSignature(true);
  };
  return (
    <div class="pl-signature-pad" role="group" aria-label={ft("forms.signature.title")}>
      <p class="pl-muted">{ft("forms.signature.hint")}</p>
      <svg class="pl-signature-pad__canvas" role="img" aria-label={ft("forms.signature.canvas")} touch-action="none"
        onPointerDown={(e) => { e.currentTarget.setPointerCapture(e.pointerId); setDrawing(true); setStrokes((s) => [...s, [pt(e)]]); }}
        onPointerMove={(e) => { if (drawing()) setStrokes((s) => [...s.slice(0, -1), [...s[s.length - 1]!, pt(e)]]); }}
        onPointerUp={() => setDrawing(false)} onPointerCancel={() => setDrawing(false)}>
        <path d={path()} fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
      </svg>
      <div class="pl-signature-pad__actions">
        <Button variant="ghost" onClick={() => setStrokes([])}>{ft("forms.signature.clear")}</Button>
        <Button variant="ghost" onClick={() => setSignaturePadOpen(false)}>{ft("forms.signature.cancel")}</Button>
        <Button variant="primary" disabled={normalize(strokes()) === null} onClick={save}>{ft("forms.signature.save")}</Button>
      </div>
    </div>
  );
}
