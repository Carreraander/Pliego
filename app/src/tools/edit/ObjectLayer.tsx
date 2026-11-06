import { createMemo, createResource, createSignal, For, Show } from "solid-js";
import type { OpenDoc } from "../../state/documents";
import { IconButton } from "../../ui";
import { cropImage, deleteObject, insertImage, loadImages, loadVectors, replaceImage, transformObject } from "./edit";
import { addBox, changed, clampBox, dragBox, HANDLES, rotateTransform, transformBetween, type Box, type Handle, type ImageObj, type ObjectKind, type Pt, type VectorObj } from "./geometry";
import { Ghost } from "./Ghost";
import { et } from "./text";

type Item = ImageObj | VectorObj;
interface Gesture { handle: Handle; start: Pt; now: Pt; box: Box }

/**
 * Capa de imágenes (`kind = "image"`) y objetos vectoriales (`"vector"`) de una página: se elige un objeto, se arrastra para
 * moverlo, se tira de un tirador para cambiar su tamaño y una barra de acciones permite girar, borrar y (imágenes) reemplazar o recortar.
 * En imágenes, arrastrar sobre una zona vacía dibuja el recuadro de una imagen nueva.
 */
export function ObjectLayer(p: { doc: OpenDoc; index: number; zoom: number; kind: ObjectKind }) {
  const [items] = createResource(() => ({ path: p.doc.path, revision: p.doc.revision, page: p.index, kind: p.kind }),
    (k) => (k.kind === "image" ? loadImages(p.doc, p.index) : loadVectors(p.doc, p.index)) as Promise<Item[]>);
  const [selected, setSelected] = createSignal<number | null>(null);
  const [gesture, setGesture] = createSignal<Gesture | null>(null);
  const [cropping, setCropping] = createSignal(false);
  const [draft, setDraft] = createSignal<[Pt, Pt] | null>(null);
  let layer!: HTMLDivElement;

  const at_ = (e: PointerEvent): Pt => { const r = layer.getBoundingClientRect(); return [(e.clientX - r.left) / p.zoom, (e.clientY - r.top) / p.zoom]; };
  const px = (b: Box) => ({ left: `${b[0] * p.zoom}px`, top: `${b[1] * p.zoom}px`, width: `${(b[2] - b[0]) * p.zoom}px`, height: `${(b[3] - b[1]) * p.zoom}px` });
  const current = createMemo(() => (items() ?? []).find((i) => i.id === selected()) ?? null);
  const shown = (i: Item): Box => { const g = gesture(); return g && selected() === i.id && !cropping() ? dragBox(g.box, g.handle, g.start, g.now) : i.bbox; };

  const begin = (e: PointerEvent, item: Item, handle: Handle) => {
    if (e.button !== 0) return;
    e.stopPropagation();
    layer.setPointerCapture(e.pointerId);
    setSelected(item.id);
    const a = at_(e);
    setGesture({ handle: cropping() ? "se" : handle, start: a, now: a, box: item.bbox });
    if (cropping()) setDraft([a, a]);
  };
  const finish = async (e: PointerEvent) => {
    const g = gesture(), d = draft(), it = current();
    setGesture(null);
    setDraft(null);
    if (g && it && cropping() && d) {
      const rect = clampBox(addBoxRaw(d[0], at_(e)), it.bbox);
      setCropping(false);
      if (rect) await cropImage(p.doc, p.index, it.id, rect);
      return;
    }
    if (g && it) {
      const to = dragBox(g.box, g.handle, g.start, g.now);
      if (changed(g.box, to)) await transformObject(p.doc, p.index, p.kind, it.id, transformBetween(g.box, to));
      return;
    }
    const n = draft();
    void n;
  };
  // Zona vacía: nada seleccionado; en imágenes, arrastre = recuadro para una imagen nueva.
  let emptyDrag = false;
  const down = (e: PointerEvent) => {
    if (e.button !== 0 || e.target !== e.currentTarget) return;
    setSelected(null);
    setCropping(false);
    if (p.kind !== "image") return;
    layer.setPointerCapture(e.pointerId);
    const a = at_(e);
    emptyDrag = true;
    setDraft([a, a]);
  };
  const move = (e: PointerEvent) => {
    const g = gesture();
    if (g) setGesture({ ...g, now: at_(e) });
    const d = draft();
    if (d) setDraft([d[0], at_(e)]);
  };
  const up = (e: PointerEvent) => {
    if (emptyDrag) {
      emptyDrag = false;
      const d = draft();
      setDraft(null);
      if (d) void insertImage(p.doc, p.index, addBox(d[0], at_(e)));
      return;
    }
    void finish(e);
  };
  const cancel = () => { setGesture(null); setDraft(null); emptyDrag = false; };
  const remove = async () => { const it = current(); if (it) { setSelected(null); await deleteObject(p.doc, p.index, p.kind, it.id); } };

  return (
    <div ref={layer} class="pl-edit-layer" data-tool={p.kind === "image" ? "image" : "object"} data-testid={`edit-${p.kind}-layer`}
      onPointerDown={down} onPointerMove={move} onPointerUp={up} onPointerCancel={cancel}
      onKeyDown={(e) => { if (e.key === "Delete" || e.key === "Backspace") void remove(); else if (e.key === "Escape") { setSelected(null); setCropping(false); } }}>
      <For each={items() ?? []}>{(it) => (
        <div class="pl-edit-object" classList={{ "is-selected": selected() === it.id }} style={px(shown(it))} tabindex="0" role="button"
          aria-label={et(p.kind === "image" ? "edit.image.item" : "edit.object.item", { n: it.id + 1 })} aria-pressed={selected() === it.id}
          onPointerDown={(e) => begin(e, it, "move")} onFocus={() => setSelected(it.id)}>
          <Show when={gesture() && selected() === it.id && !cropping() && changed(it.bbox, shown(it))}>
            <Ghost doc={p.doc} index={p.index} zoom={p.zoom} from={it.bbox} to={shown(it)} />
          </Show>
          <Show when={selected() === it.id && !cropping()}>
            <For each={HANDLES}>{(h) => <span class={`pl-edit-handle pl-edit-handle--${h}`} onPointerDown={(e) => begin(e, it, h)} />}</For>
          </Show>
        </div>
      )}</For>
      <Show when={draft()} keyed>{(d) => <div class="pl-edit-preview" style={px(cropping() ? addBoxRaw(d[0], d[1]) : addBox(d[0], d[1]))} />}</Show>
      <Show when={current()} keyed>{(it) => (
        <div class="pl-edit-actions" style={{ left: `${shown(it)[0] * p.zoom}px`, top: `${shown(it)[3] * p.zoom + 6}px` }} onPointerDown={(e) => e.stopPropagation()}>
          <Show when={p.kind === "image"}>
            <IconButton icon="refresh" label={et("edit.image.replace")} onClick={() => void replaceImage(p.doc, p.index, it.id)} />
            <IconButton icon="scissors" label={et("edit.image.crop")} aria-pressed={cropping()} onClick={() => setCropping(!cropping())} />
          </Show>
          <IconButton icon="rotate-cw" label={et("edit.object.rotate")} onClick={() => void transformObject(p.doc, p.index, p.kind, it.id, rotateTransform(90))} />
          <IconButton icon="trash" label={et("edit.object.delete")} onClick={() => void remove()} />
          <Show when={cropping()}><span class="pl-edit-hint">{et("edit.image.cropHint")}</span></Show>
        </div>
      )}</Show>
    </div>
  );
}

/** Caja entre dos puntos sin recuadro mínimo (el recorte puede ser pequeño). */
function addBoxRaw(a: Pt, b: Pt): Box {
  return [Math.min(a[0], b[0]), Math.min(a[1], b[1]), Math.max(a[0], b[0]), Math.max(a[1], b[1])];
}
