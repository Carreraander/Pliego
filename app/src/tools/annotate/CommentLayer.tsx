import "./annotate.css";
import { createSignal, Show } from "solid-js";
import { activeMode } from "../../shell";
import { Button } from "../../ui";
import type { PageOverlay } from "../../viewer/overlays";
import { addComment } from "./edit";
import { specFor, thin, type Pt } from "./geometry";
import { commentColor, commentTool } from "./state";
import { at } from "./text";

/** Capa de una página: recoge el gesto (arrastrar, clic o trazo) y crea el comentario en coordenadas de página. */
export const CommentLayer: PageOverlay = (p) => {
  const [points, setPoints] = createSignal<Pt[]>([]);
  const [pendingNote, setPendingNote] = createSignal<Pt | null>(null);
  const [text, setText] = createSignal("");
  // Solo recoge gestos con el modo Comentar activo.
  const tool = () => (activeMode()?.id === "annotate.comment" ? commentTool() : null);

  // `offsetX/Y` va en el sistema local del elemento, así que sirve aunque la vista esté girada.
  const at_ = (e: PointerEvent): Pt => [e.offsetX / p.zoom, e.offsetY / p.zoom];

  const finish = async () => {
    const t = tool();
    const pts = points();
    setPoints([]);
    if (!t || !pts.length) return;
    if (t === "note") { setPendingNote(pts[0]!); setText(""); return; }
    const spec = specFor(t, p.index, commentColor(), { points: t === "ink" ? thin(pts) : pts });
    if (spec) await addComment(p.doc, spec);
  };
  const saveNote = async () => {
    const pos = pendingNote();
    if (!pos) return;
    const spec = specFor("note", p.index, commentColor(), { points: [pos] }, text());
    setPendingNote(null);
    if (spec) await addComment(p.doc, spec);
  };

  const preview = () => {
    const pts = points();
    const t = tool();
    if (!t || pts.length < 1 || t === "note") return null;
    const first = pts[0]!, last = pts[pts.length - 1]!;
    if (t === "ink") return { path: pts.map((q, i) => `${i ? "L" : "M"}${q[0] * p.zoom} ${q[1] * p.zoom}`).join(" ") };
    return { x: Math.min(first[0], last[0]) * p.zoom, y: Math.min(first[1], last[1]) * p.zoom, w: Math.abs(first[0] - last[0]) * p.zoom, h: Math.abs(first[1] - last[1]) * p.zoom };
  };

  return (
    <Show when={tool()}>
      <div class="pl-comment-layer" data-tool={tool()} data-testid="comment-layer"
        onPointerDown={(e) => { if (e.button !== 0 || e.target !== e.currentTarget) return; e.currentTarget.setPointerCapture(e.pointerId); setPendingNote(null); setPoints([at_(e)]); }}
        onPointerMove={(e) => { if (points().length) setPoints((all) => (tool() === "ink" ? [...all, at_(e)] : [all[0]!, at_(e)])); }}
        onPointerUp={(e) => { if (!points().length) return; if (tool() !== "ink") setPoints((all) => [all[0]!, at_(e)]); void finish(); }}
        onPointerCancel={() => setPoints([])}>
        <Show when={preview()} keyed>{(v) => "path" in v
          ? <svg class="pl-comment-preview"><path d={v.path} fill="none" stroke="currentColor" stroke-width="2" /></svg>
          : <div class="pl-comment-preview pl-comment-preview--box" style={{ left: `${v.x}px`, top: `${v.y}px`, width: `${v.w}px`, height: `${v.h}px` }} />}
        </Show>
        <Show when={pendingNote()} keyed>{(pos) => (
          <form class="pl-comment-note" style={{ left: `${pos[0] * p.zoom}px`, top: `${pos[1] * p.zoom}px` }}
            onSubmit={(e) => { e.preventDefault(); void saveNote(); }} onKeyDown={(e) => { if (e.key === "Escape") setPendingNote(null); }}>
            <textarea autofocus rows="3" aria-label={at("annotate.note.label")} value={text()} onInput={(e) => setText(e.currentTarget.value)} />
            <div class="pl-comment-note__actions">
              <Button variant="ghost" type="button" onClick={() => setPendingNote(null)}>{at("annotate.note.cancel")}</Button>
              <Button variant="primary" type="submit">{at("annotate.note.save")}</Button>
            </div>
          </form>
        )}</Show>
      </div>
    </Show>
  );
};
