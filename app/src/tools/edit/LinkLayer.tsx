import { createResource, createSignal, For, Show } from "solid-js";
import type { OpenDoc } from "../../state/documents";
import { Button, Field, TextInput } from "../../ui";
import { addLink, deleteLink, loadLinks, updateLink } from "./edit";
import { addBox, parseTarget, targetText, type Box, type LinkItem, type Pt } from "./geometry";
import { et } from "./text";

type Target = { kind: "new"; rect: Box } | { kind: "link"; link: LinkItem };

/** Capa de enlaces de una página: muestra los que hay (clic = cambiar el destino o borrar) y permite dibujar uno nuevo arrastrando. */
export function LinkLayer(p: { doc: OpenDoc; index: number; zoom: number }) {
  const [links] = createResource(() => ({ path: p.doc.path, revision: p.doc.revision, page: p.index }), () => loadLinks(p.doc, p.index));
  const [drag, setDrag] = createSignal<[Pt, Pt] | null>(null);
  const [target, setTarget] = createSignal<Target | null>(null);
  let layer!: HTMLDivElement;
  const at_ = (e: PointerEvent): Pt => { const r = layer.getBoundingClientRect(); return [(e.clientX - r.left) / p.zoom, (e.clientY - r.top) / p.zoom]; };
  const px = (b: Box) => ({ left: `${b[0] * p.zoom}px`, top: `${b[1] * p.zoom}px`, width: `${(b[2] - b[0]) * p.zoom}px`, height: `${(b[3] - b[1]) * p.zoom}px` });
  const box = (t: Target) => (t.kind === "new" ? t.rect : t.link.rect);

  return (
    <div ref={layer} class="pl-edit-layer" data-tool="link" data-testid="edit-link-layer"
      onPointerDown={(e) => {
        if (e.button !== 0 || e.target !== e.currentTarget) return;
        setTarget(null);
        layer.setPointerCapture(e.pointerId);
        const a = at_(e);
        setDrag([a, a]);
      }}
      onPointerMove={(e) => { const d = drag(); if (d) setDrag([d[0], at_(e)]); }}
      onPointerUp={(e) => { const d = drag(); setDrag(null); if (d) setTarget({ kind: "new", rect: addBox(d[0], at_(e)) }); }}
      onPointerCancel={() => setDrag(null)}>
      <For each={links() ?? []}>{(l) => (
        <button type="button" class="pl-edit-object pl-edit-link" style={px(l.rect)} aria-label={et("edit.link.item", { target: targetText(l.target) || et("edit.link.other") })}
          onPointerDown={(e) => e.stopPropagation()} onClick={() => setTarget({ kind: "link", link: l })} />
      )}</For>
      <Show when={drag()} keyed>{(d) => <div class="pl-edit-preview" style={px(addBox(d[0], d[1]))} />}</Show>
      <Show when={target()} keyed>{(t) => (
        <LinkForm pages={p.doc.sizes.length} link={t.kind === "link" ? t.link : null} style={{ left: `${box(t)[0] * p.zoom}px`, top: `${box(t)[3] * p.zoom + 6}px` }}
          onCancel={() => setTarget(null)}
          onApply={(to) => void (t.kind === "new" ? addLink(p.doc, p.index, t.rect, to) : updateLink(p.doc, p.index, t.link.id, to)).then((ok) => ok && setTarget(null))}
          onDelete={t.kind === "link" ? () => void deleteLink(p.doc, p.index, t.link.id).then((ok) => ok && setTarget(null)) : undefined} />
      )}</Show>
    </div>
  );
}

function LinkForm(p: { pages: number; link: LinkItem | null; style: { left: string; top: string }; onApply: (t: NonNullable<ReturnType<typeof parseTarget>>) => void; onCancel: () => void; onDelete?: () => void }) {
  const [text, setText] = createSignal(targetText(p.link?.target ?? null));
  const parsed = () => parseTarget(text(), p.pages);
  return (
    <form class="pl-edit-form" style={p.style} onPointerDown={(e) => e.stopPropagation()}
      onSubmit={(e) => { e.preventDefault(); const t = parsed(); if (t) p.onApply(t); }} onKeyDown={(e) => { if (e.key === "Escape") p.onCancel(); }}>
      <Field label={et("edit.link.target")} hint={et("edit.link.hint", { n: p.pages })} error={text().trim() && !parsed() ? et("edit.link.bad", { n: p.pages }) : undefined}>
        <TextInput autofocus value={text()} onInput={(e) => setText(e.currentTarget.value)} />
      </Field>
      <div class="pl-edit-form__actions">
        <Show when={p.onDelete}><Button variant="ghost" type="button" onClick={p.onDelete}>{et("edit.link.delete")}</Button></Show>
        <Button variant="ghost" type="button" onClick={p.onCancel}>{et("edit.editor.cancel")}</Button>
        <Button variant="primary" type="submit" disabled={!parsed()}>{et("edit.editor.apply")}</Button>
      </div>
    </form>
  );
}
