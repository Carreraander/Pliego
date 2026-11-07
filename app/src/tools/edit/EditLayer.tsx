import "./edit.css";
import { createResource, createSignal, For, Match, Show, Switch } from "solid-js";
import { activeMode } from "../../shell";
import type { PageOverlay } from "../../viewer/overlays";
import { insertText, loadBlocks, replaceText } from "./edit";
import { addBox, boxOf, type Box, type FormAnchor, type Pt, type TextBlock } from "./geometry";
import { editTool, reflow } from "./state";
import { LinkLayer } from "./LinkLayer";
import { ObjectLayer } from "./ObjectLayer";
import { TextEditor } from "./TextEditor";
import { et } from "./text";

type Target = { kind: "block"; block: TextBlock } | { kind: "new"; rect: Box };

/** Capa de una página en el modo Editar: según la herramienta, texto (párrafos y texto nuevo), imágenes, objetos vectoriales o enlaces. */
export const EditLayer: PageOverlay = (p) => {
  const active = () => activeMode()?.id === "edit.content";
  return (
    <Show when={active()}>
      <Switch fallback={<TextLayer {...p} />}>
        <Match when={editTool() === "image"}><ObjectLayer doc={p.doc} index={p.index} zoom={p.zoom} kind="image" /></Match>
        <Match when={editTool() === "object"}><ObjectLayer doc={p.doc} index={p.index} zoom={p.zoom} kind="vector" /></Match>
        <Match when={editTool() === "link"}><LinkLayer doc={p.doc} index={p.index} zoom={p.zoom} /></Match>
      </Switch>
    </Show>
  );
};

/** Texto: recuadros de los párrafos, arrastre para texto nuevo y el formulario de edición. */
const TextLayer: PageOverlay = (p) => {
  const active = () => activeMode()?.id === "edit.content";
  const [blocks] = createResource(() => (active() && editTool() === "text" ? { path: p.doc.path, revision: p.doc.revision, page: p.index } : null), () => loadBlocks(p.doc, p.index));
  const [drag, setDrag] = createSignal<[Pt, Pt] | null>(null);
  const [target, setTarget] = createSignal<Target | null>(null);
  const at_ = (e: PointerEvent): Pt => [e.offsetX / p.zoom, e.offsetY / p.zoom];
  const px = (b: Box) => ({ left: `${b[0] * p.zoom}px`, top: `${b[1] * p.zoom}px`, width: `${(b[2] - b[0]) * p.zoom}px`, height: `${(b[3] - b[1]) * p.zoom}px` });
  const corner = (b: Box): FormAnchor => ({ x: b[0] * p.zoom, below: b[3] * p.zoom + 6, above: b[1] * p.zoom - 6 });
  const preview = () => { const d = drag(); return d ? boxOf(d[0], d[1]) : null; };

  const apply = async (text: string, style: Parameters<typeof replaceText>[3]) => {
    const t = target();
    if (!t) return;
    const ok = t.kind === "block" ? await replaceText(p.doc, t.block, text, style, reflow()) : await insertText(p.doc, p.index, t.rect, text, style);
    if (ok) setTarget(null);
  };

  return (
    <Show when={active()}>
      <div class="pl-edit-layer" data-tool={editTool()} data-testid="edit-layer"
        onPointerDown={(e) => {
          if (e.button !== 0 || e.target !== e.currentTarget) return;
          setTarget(null);
          if (editTool() !== "add") return;
          e.currentTarget.setPointerCapture(e.pointerId);
          const a = at_(e);
          setDrag([a, a]);
        }}
        onPointerMove={(e) => { const d = drag(); if (d) setDrag([d[0], at_(e)]); }}
        onPointerUp={(e) => { const d = drag(); setDrag(null); if (d) setTarget({ kind: "new", rect: addBox(d[0], at_(e)) }); }}
        onPointerCancel={() => setDrag(null)}>
        <Show when={editTool() === "text"}>
          <For each={blocks() ?? []}>{(b) => (
            <button type="button" class="pl-edit-block" style={px(b.bbox)} aria-label={et("edit.block", { text: b.text.slice(0, 60) })}
              onPointerDown={(e) => e.stopPropagation()} onClick={() => setTarget({ kind: "block", block: b })} />
          )}</For>
        </Show>
        <Show when={preview()} keyed>{(b) => <div class="pl-edit-preview" style={px(b)} />}</Show>
        <Show when={target()} keyed>{(t) => (
          <TextEditor block={t.kind === "block" ? t.block : null} anchor={corner(t.kind === "block" ? t.block.bbox : t.rect)}
            onApply={(text, style) => void apply(text, style)} onCancel={() => setTarget(null)} />
        )}</Show>
      </div>
    </Show>
  );
};
