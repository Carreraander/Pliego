import "./compare.css";
import { createMemo, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { api, tileUrl, type DocId, type PageSize } from "../../api";
import { Checkbox, Icon, IconButton, ToolbarSeparator } from "../../ui";
import { clampZoom, currentPage, lowResZoom, stepZoom, TILE_SIZE, tilesFor, visiblePages } from "../../viewer/layout";
import { fileName } from "../run";
import { cmt } from "./strings";
import {
  changeStops, computeRows, currentStop, fitColumnsZoom, LEGEND, READ_OFFSET, rectStyle, sizeOf, stepStop, stopPx,
  type Comparison, type PageComparison,
} from "./view";

/** Documento de un lado de la vista: abierto en el servicio, así sus páginas se dibujan con las teselas de siempre. */
export interface SideDoc { id: DocId; path: string; sizes: PageSize[]; revision: number }

export interface CompareViewProps { comparison: Comparison; a: SideDoc; b: SideDoc; onClose: () => void }

/** Altura de la cabecera fija con los nombres de los dos documentos. */
const HEAD_H = 32;
/** Redondea el zoom para que las teselas se reutilicen (la caché del servidor usa el zoom en la clave). */
const q = (z: number) => Math.round(z * 1000) / 1000;

/**
 * Comparar, vista lado a lado: el original a la izquierda y la versión nueva a la derecha en un solo lienzo con un solo
 * desplazamiento, de modo que las dos columnas siempre enseñan las páginas emparejadas. Los cambios del texto salen
 * resaltados en los dos documentos (rojo lo quitado, verde lo añadido, ámbar lo cambiado) y las zonas que solo se
 * distinguen por el aspecto, con un recuadro.
 */
export default function CompareView(props: CompareViewProps) {
  let box!: HTMLDivElement;
  const rows = () => props.comparison.pages;
  const [zoom, setZoom] = createSignal(1);
  const [scrollTop, setScrollTop] = createSignal(0);
  const [size, setSize] = createSignal({ width: 1000, height: 600 });
  const [showText, setShowText] = createSignal(true);
  const [showVisual, setShowVisual] = createSignal(true);

  const layout = createMemo(() => computeRows(rows(), props.a.sizes, props.b.sizes, zoom()));
  const visible = createMemo(() => visiblePages(layout(), scrollTop(), size().height));
  const stops = createMemo(() => changeStops(props.comparison));
  const pos = () => scrollTop() - HEAD_H + READ_OFFSET;
  // Cambio al que se ha saltado con las flechas o F3. Vale mientras no se desplace a mano (el final del documento
  // puede impedir llegar a la marca de lectura, y entonces la posición sola no dice en qué cambio se está).
  const [cursor, setCursor] = createSignal<{ index: number; scrollTop: number } | null>(null);
  const at = createMemo(() => cursor()?.index ?? currentStop(stops(), layout().tops, zoom(), pos()));
  const rowNow = () => currentPage(layout(), scrollTop() - HEAD_H, size().height);
  const contentWidth = () => Math.max(layout().columnWidth * 2 + 1, size().width);

  const fit = () => { setCursor(null); setZoom(q(fitColumnsZoom(rows(), props.a.sizes, props.b.sizes, size().width))); };
  onMount(() => {
    const measure = () => setSize({ width: box.clientWidth, height: box.clientHeight });
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(box);
    onCleanup(() => ro.disconnect());
    fit();
    box.focus();
    // Empieza en el primer cambio, que es a lo que se viene.
    requestAnimationFrame(() => go(1, true));
  });

  /** Salta al cambio siguiente o anterior. Con `first`, al primero si aún no se ha movido. */
  const go = (dir: 1 | -1, first = false) => {
    const c = cursor();
    let i: number | null;
    if (c && !first) i = c.index + dir >= 0 && c.index + dir < stops().length ? c.index + dir : null;
    else i = stepStop(stops(), layout().tops, zoom(), first ? -1e9 : pos(), dir);
    if (i === null) return;
    box.scrollTop = Math.max(0, HEAD_H + stopPx(stops()[i]!, layout().tops, zoom()) - READ_OFFSET);
    setCursor({ index: i, scrollTop: box.scrollTop });
  };
  const onScroll = () => {
    setScrollTop(box.scrollTop);
    const c = cursor();
    if (c && Math.abs(box.scrollTop - c.scrollTop) > 3) setCursor(null);
  };
  const zoomTo = (z: number) => {
    // Mantiene a la vista lo mismo al cambiar de zoom.
    const ratio = clampZoom(z) / zoom();
    const top = box.scrollTop;
    setCursor(null);
    setZoom(q(clampZoom(z)));
    box.scrollTop = top * ratio;
  };
  const onKey = (e: KeyboardEvent) => {
    if (e.key === "F3") { e.preventDefault(); go(e.shiftKey ? -1 : 1); }
    else if (e.key === "Escape") props.onClose();
    else if ((e.ctrlKey || e.metaKey) && (e.key === "+" || e.key === "=")) { e.preventDefault(); zoomTo(stepZoom(zoom(), 1)); }
    else if ((e.ctrlKey || e.metaKey) && e.key === "-") { e.preventDefault(); zoomTo(stepZoom(zoom(), -1)); }
  };

  const s = () => props.comparison.summary;
  const total = () => stops().length;
  return (
    <div class="cmp" data-testid="compare-view" tabindex={-1} onKeyDown={onKey}>
      <div class="cmp-bar" role="toolbar" aria-label={cmt("view.title")}>
        <IconButton icon="chevron-up" label={cmt("view.prev")} disabled={total() === 0} onClick={() => go(-1)} />
        <IconButton icon="chevron-down" label={cmt("view.next")} disabled={total() === 0} onClick={() => go(1)} />
        <span class="cmp-count pl-num" aria-live="polite">{total() === 0 ? cmt("view.none") : cmt("view.count", { n: (at() ?? -1) + 1, total: total() })}</span>
        <ToolbarSeparator />
        <Checkbox label={cmt("view.showText")} checked={showText()} onChange={(e) => setShowText(e.currentTarget.checked)} />
        <Checkbox label={cmt("view.showVisual")} checked={showVisual()} onChange={(e) => setShowVisual(e.currentTarget.checked)} />
        <span class="cmp-spacer" />
        <IconButton icon="zoom-out" label={cmt("view.zoomOut")} onClick={() => zoomTo(stepZoom(zoom(), -1))} />
        <span class="pl-num pl-muted cmp-zoom">{Math.round(zoom() * 100)} %</span>
        <IconButton icon="zoom-in" label={cmt("view.zoomIn")} onClick={() => zoomTo(stepZoom(zoom(), 1))} />
        <IconButton icon="maximize" label={cmt("view.fit")} onClick={fit} />
        <ToolbarSeparator />
        <IconButton icon="x" label={cmt("view.close")} onClick={() => props.onClose()} />
      </div>
      <div class="cmp-summary">
        <p class="pl-muted" data-testid="compare-summary">
          {cmt("view.summary", { pages: s().changed_pages, inserted: s().inserted_pages, deleted: s().deleted_pages, added: s().added_words, removed: s().removed_words, changed: s().changed_words })}
        </p>
        <ul class="cmp-legend" aria-label={cmt("view.legend")}>
          <For each={LEGEND}>{(k) => <li><i class={`cmp-swatch cmp-${k}`} />{cmt(`view.kind.${k}`)}</li>}</For>
          <li><i class="cmp-swatch cmp-visual" />{cmt("view.kind.visual")}</li>
        </ul>
      </div>
      <div class="cmp-scroll" ref={box} tabindex={0} onScroll={onScroll} data-testid="compare-scroll">
        <div class="cmp-heads" style={{ width: `${contentWidth()}px`, height: `${HEAD_H}px` }}>
          <div class="cmp-head"><Icon name="file-text" size={16} /><b>{cmt("view.original")}</b><span title={props.a.path}>{fileName(props.a.path)}</span></div>
          <div class="cmp-head"><Icon name="file-text" size={16} /><b>{cmt("view.newer")}</b><span title={props.b.path}>{fileName(props.b.path)}</span></div>
        </div>
        <div class="cmp-content" style={{ height: `${layout().totalHeight}px`, width: `${contentWidth()}px` }}>
          <For each={visible()}>{(i) => (
            <Row index={i} row={rows()[i]!} a={props.a} b={props.b} zoom={zoom()} top={layout().tops[i]!} height={layout().boxes[i]!.height}
              column={contentWidth() / 2} scrollTop={scrollTop() - HEAD_H} viewportHeight={size().height} showText={showText()} showVisual={showVisual()} />
          )}</For>
        </div>
      </div>
      <span class="cmp-sr" aria-live="polite">{cmt("view.row", { n: rowNow() + 1, total: rows().length })}</span>
    </div>
  );
}

function Row(p: {
  index: number; row: PageComparison; a: SideDoc; b: SideDoc; zoom: number; top: number; height: number; column: number;
  scrollTop: number; viewportHeight: number; showText: boolean; showVisual: boolean;
}) {
  const label = () => {
    const n = (v: number | null) => (v === null ? "–" : String(v + 1));
    return `${n(p.row.page_a)} / ${n(p.row.page_b)}`;
  };
  return (
    <div class="cmp-row" classList={{ "cmp-row--diff": p.row.status !== "matched" }} style={{ top: `${p.top}px`, height: `${p.height}px`, width: `${p.column * 2}px` }} data-row={p.index} data-status={p.row.status}>
      <div class="cmp-cell" style={{ width: `${p.column}px` }} data-side="a">
        <PageCell side="a" doc={p.a} page={p.row.page_a} missing={p.row.status === "inserted"} row={p.row} zoom={p.zoom} top={p.top} height={p.height} scrollTop={p.scrollTop} viewportHeight={p.viewportHeight} showText={p.showText} showVisual={p.showVisual} />
      </div>
      <div class="cmp-cell" style={{ width: `${p.column}px` }} data-side="b">
        <PageCell side="b" doc={p.b} page={p.row.page_b} missing={p.row.status === "deleted"} row={p.row} zoom={p.zoom} top={p.top} height={p.height} scrollTop={p.scrollTop} viewportHeight={p.viewportHeight} showText={p.showText} showVisual={p.showVisual} />
      </div>
      <span class="cmp-pageno pl-num" aria-hidden="true">{label()}</span>
    </div>
  );
}

function PageCell(p: {
  side: "a" | "b"; doc: SideDoc; page: number | null; row: PageComparison; missing: boolean; zoom: number; top: number; height: number;
  scrollTop: number; viewportHeight: number; showText: boolean; showVisual: boolean;
}) {
  const raw = () => sizeOf(p.doc.sizes, p.page);
  const w = () => (raw()?.width ?? 0) * p.zoom;
  const h = () => (raw()?.height ?? 0) * p.zoom;
  const tiles = createMemo(() => {
    const r = raw();
    if (!r) return [];
    const vis = { left: 0, right: w(), top: Math.max(0, p.scrollTop - 200 - p.top), bottom: Math.min(h(), p.scrollTop + p.viewportHeight + 200 - p.top) };
    return vis.bottom <= vis.top ? [] : tilesFor({ width: w(), height: h() }, vis);
  });
  const low = () => q(lowResZoom(raw() ?? { width: 1, height: 1 }));
  const rects = createMemo(() => {
    const out: { kind: string; rect: Parameters<typeof rectStyle>[0]; title: string }[] = [];
    if (p.showText) {
      for (const t of p.row.text_changes) {
        const list = p.side === "a" ? t.rects_a : t.rects_b;
        const kind = t.format_only ? "format" : t.kind;
        const title = t.format_only ? (t.note ?? "") : p.side === "a" ? t.text_a : t.text_b;
        for (const r of list) out.push({ kind, rect: r, title });
      }
    }
    if (p.showVisual) for (const v of p.row.visual_regions) out.push({ kind: "visual", rect: v.rect, title: cmt("view.kind.visual") });
    return out;
  });
  return (
    <Show when={raw() && !p.missing && p.page !== null} fallback={
      <div class="cmp-gap" style={{ height: `${p.height}px` }} data-testid="compare-gap">
        <span>{cmt(p.row.status === "inserted" ? "view.gap.inserted" : "view.gap.deleted")}</span>
      </div>
    }>
      <div class="cmp-page" style={{ width: `${w()}px`, height: `${h()}px` }} data-page={p.page!}>
        <img class="tile low" alt="" draggable={false} src={tileUrl(p.doc.id, p.page!, low(), 0, 0, p.doc.revision)} style={{ width: `${w()}px`, height: `${h()}px` }} />
        <For each={tiles()}>{(t) => (
          <img class="tile" alt="" draggable={false} src={tileUrl(p.doc.id, p.page!, q(p.zoom), t.col, t.row, p.doc.revision)}
            style={{ left: `${t.col * TILE_SIZE}px`, top: `${t.row * TILE_SIZE}px` }} />
        )}</For>
        <For each={rects()}>{(r) => <div class={`cmp-mark cmp-${r.kind}`} style={rectStyle(r.rect, p.zoom)} title={r.title} data-kind={r.kind} />}</For>
      </div>
    </Show>
  );
}

/** Abre el segundo documento en el servicio (sin pestaña) para dibujar sus páginas. */
export async function openSideDoc(path: string, password?: string | null): Promise<SideDoc> {
  const id = await api.openDocument(path, password ?? undefined);
  try {
    return { id, path, sizes: await api.pageSizes(id), revision: 0 };
  } catch (e) {
    await api.closeDocument(id).catch(() => undefined);
    throw e;
  }
}
