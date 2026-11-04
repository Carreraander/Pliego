import "./organize.css";
import { For, Show, createSignal } from "solid-js";
import { tileUrl } from "../../api";
import { activeDoc } from "../../state/documents";
import { EmptyState } from "../../ui";
import { clearSelection, deleteSelected, movePage, select, selectAll } from "./pageedits";
import { editsOf, updateEdits } from "./store";
import { ot } from "./text";

const THUMB = 150;

/** Cuadrícula de miniaturas del modo Organizar: seleccionar, arrastrar para reordenar y teclas Supr / Ctrl+A / Esc. */
export default function OrganizeGrid() {
  const [dragFrom, setDragFrom] = createSignal<number | null>(null);
  const doc = () => activeDoc();
  const pages = () => doc()?.sizes.length ?? 0;
  const edits = () => editsOf(doc()?.id ?? null, pages());
  const update = (f: Parameters<typeof updateEdits>[2]) => { const d = doc(); if (d) updateEdits(d.id, pages(), f); };

  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Delete") { update(deleteSelected); e.preventDefault(); }
    else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "a") { update(selectAll); e.preventDefault(); }
    else if (e.key === "Escape") update(clearSelection);
  };

  return (
    <Show when={doc()} keyed fallback={<EmptyState icon="layout-grid" title={ot("organize.grid.empty")} />}>
      {(d) => (
        <div class="pl-organize-grid" role="listbox" aria-multiselectable="true" aria-label={ot("organize.grid.label")} onKeyDown={onKey}>
          <For each={edits().items}>{(item, i) => {
            const size = () => d.sizes[item.page] ?? { width: 595, height: 842 };
            const zoom = () => Math.round((THUMB / Math.max(size().width, size().height)) * 1000) / 1000;
            const turned = () => item.rotate % 180 !== 0;
            const w = () => Math.round(size().width * zoom());
            const h = () => Math.round(size().height * zoom());
            return (
              <button
                type="button" role="option" class="pl-organize-card" classList={{ "is-selected": edits().selected.includes(i()), "is-drop": dragFrom() !== null && dragFrom() !== i() }}
                aria-selected={edits().selected.includes(i())} draggable="true"
                onClick={(e) => update((s) => select(s, i(), e.shiftKey ? "range" : e.ctrlKey || e.metaKey ? "toggle" : "single"))}
                onDragStart={() => setDragFrom(i())} onDragEnd={() => setDragFrom(null)} onDragOver={(e) => e.preventDefault()}
                onDrop={(e) => { e.preventDefault(); const from = dragFrom(); setDragFrom(null); if (from !== null) update((s) => movePage(s, from, i())); }}
              >
                <span class="pl-organize-thumb" style={{ width: `${turned() ? h() : w()}px`, height: `${turned() ? w() : h()}px` }}>
                  <img alt="" draggable={false} loading="lazy" width={w()} height={h()} src={tileUrl(d.id, item.page, zoom(), 0, 0, d.revision)}
                    style={{ transform: `rotate(${item.rotate}deg)` }} />
                </span>
                <span class="pl-organize-num pl-num">{i() + 1}{item.page !== i() ? <span class="pl-muted"> ({ot("organize.grid.was", { n: item.page + 1 })})</span> : null}</span>
              </button>
            );
          }}</For>
        </div>
      )}
    </Show>
  );
}
