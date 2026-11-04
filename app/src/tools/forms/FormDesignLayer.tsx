import "./forms.css";
import { createEffect, createResource, createSignal, For, Show } from "solid-js";
import type { PageOverlay } from "../../viewer/overlays";
import { loadFields } from "./edit";
import { boxOf, newBox, type Box, type FieldInfo, type FieldKind, type Pt } from "./geometry";
import { designOpen, designTool, selectedField, setEditing, setSelectedField } from "./state";
import { ft } from "./text";

/** Capa de una página en el modo Diseñar formulario: muestra los campos, recoge el arrastre que dibuja uno nuevo. */
export const FormDesignLayer: PageOverlay = (p) => {
  const active = () => designOpen();
  const [fields] = createResource(() => (active() ? { path: p.doc.path, revision: p.doc.revision, id: p.doc.id } : null), () => loadFields(p.doc));
  const here = () => (fields() ?? []).filter((f) => f.page === p.index);
  const [drag, setDrag] = createSignal<[Pt, Pt] | null>(null);
  const at_ = (e: PointerEvent): Pt => [e.offsetX / p.zoom, e.offsetY / p.zoom];

  const edit = (f: FieldInfo) => {
    setSelectedField(f.name);
    setEditing({ mode: "edit", doc: p.doc, field: f });
  };
  const finish = (a: Pt, b: Pt) => {
    const kind = designTool();
    if (kind === "select") return;
    const rect = newBox(kind as FieldKind, a, b);
    setSelectedField(null);
    setEditing({ mode: "new", doc: p.doc, kind: kind as FieldKind, page: p.index, rect });
  };
  // Al salir del modo o cambiar de herramienta se suelta el campo elegido.
  createEffect(() => { if (!active() || designTool() !== "select") setSelectedField(null); });

  const px = (b: Box) => ({ left: `${b[0] * p.zoom}px`, top: `${b[1] * p.zoom}px`, width: `${(b[2] - b[0]) * p.zoom}px`, height: `${(b[3] - b[1]) * p.zoom}px` });
  const preview = () => { const d = drag(); return d ? boxOf(d[0], d[1]) : null; };

  return (
    <Show when={active()}>
      <div class="pl-forms-layer" data-tool={designTool()} data-testid="forms-layer"
        onPointerDown={(e) => { if (e.button !== 0 || e.target !== e.currentTarget) return; if (designTool() === "select") { setSelectedField(null); return; } e.currentTarget.setPointerCapture(e.pointerId); const a = at_(e); setDrag([a, a]); }}
        onPointerMove={(e) => { const d = drag(); if (d) setDrag([d[0], at_(e)]); }}
        onPointerUp={(e) => { const d = drag(); setDrag(null); if (d) finish(d[0], at_(e)); }}
        onPointerCancel={() => setDrag(null)}>
        <For each={here()}>{(f) => (
          <button type="button" class="pl-forms-box" classList={{ "pl-forms-box--selected": selectedField() === f.name }} style={px(f.rect)}
            aria-label={ft("forms.box", { name: f.name, kind: ft("forms.tool." + f.kind) })} aria-pressed={selectedField() === f.name}
            onPointerDown={(e) => { if (designTool() === "select") e.stopPropagation(); }} onClick={() => { if (designTool() === "select") edit(f); }}>
            <span class="pl-forms-box__name">{f.name}</span>
          </button>
        )}</For>
        <Show when={preview()} keyed>{(b) => <div class="pl-forms-preview" style={px(b)} />}</Show>
      </div>
    </Show>
  );
};
