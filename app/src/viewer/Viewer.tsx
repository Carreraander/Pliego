import { createEffect, createMemo, createSignal, For, onCleanup, onMount } from "solid-js";
import { tileUrl } from "../api";
import { currentPageIndex, setCurrentPageIndex, type OpenDoc } from "../state/documents";
import { clearGoTo, goToRequest } from "../state/navigation";
import { matchIndex, matchesOnPage } from "../state/search";
import { pageOverlays } from "./overlays";
import {
  computeLayout, currentPage, lowResZoom, rotationTransform, TILE_SIZE, tilesFor, unrotateRect, visiblePages,
  type Rotation,
} from "./layout";

/** Redondea el zoom para que las teselas se reutilicen (la caché del servidor usa el zoom en la clave). */
const q = (z: number) => Math.round(z * 1000) / 1000;

export function Viewer(props: { doc: OpenDoc; zoom: number; rotation: Rotation }) {
  let box!: HTMLDivElement;
  const [scrollTop, setScrollTop] = createSignal(0);
  const [size, setSize] = createSignal({ width: 800, height: 600 });

  const layout = createMemo(() => computeLayout(props.doc.sizes, props.zoom, props.rotation));
  const visible = createMemo(() => visiblePages(layout(), scrollTop(), size().height));

  const update = () => {
    setScrollTop(box.scrollTop);
    setCurrentPageIndex(currentPage(layout(), box.scrollTop, size().height));
  };
  onMount(() => {
    const ro = new ResizeObserver(() => setSize({ width: box.clientWidth, height: box.clientHeight }));
    ro.observe(box);
    onCleanup(() => ro.disconnect());
    // Al volver a esta pestaña o cambiar la página desde fuera, mantiene la posición.
    box.scrollTop = layout().tops[currentPageIndex()] ?? 0;
  });

  // Atiende "ir a página" (miniaturas, marcadores, búsqueda).
  createEffect(() => {
    const req = goToRequest();
    if (!req || !box) return;
    const size = props.doc.sizes[req.page];
    if (!size) return clearGoTo();
    const top = layout().tops[req.page] ?? 0;
    // El desplazamiento vertical interno solo es fiable sin rotación; con rotación se va al inicio de la página.
    box.scrollTop = Math.max(0, top - 8 + (props.rotation === 0 ? (req.y ?? 0) * props.zoom : 0));
    clearGoTo();
  });

  return (
    <div class="viewer" ref={box} onScroll={update} data-testid="viewer">
      <div class="viewer-content" style={{ height: `${layout().totalHeight}px`, width: `${layout().maxWidth + 24}px` }}>
        <For each={visible()}>{(i) => <PageView doc={props.doc} index={i} zoom={props.zoom} rotation={props.rotation} layout={layout()} scrollTop={scrollTop()} viewport={size()} />}</For>
      </div>
    </div>
  );
}

function PageView(props: {
  doc: OpenDoc; index: number; zoom: number; rotation: Rotation;
  layout: ReturnType<typeof computeLayout>; scrollTop: number; viewport: { width: number; height: number };
}) {
  const raw = () => props.doc.sizes[props.index]!;
  const unrotated = () => ({ width: raw().width * props.zoom, height: raw().height * props.zoom });
  const box = () => props.layout.boxes[props.index]!;
  const top = () => props.layout.tops[props.index]!;

  // Zona visible dentro de la caja de la página (coordenadas de la caja girada) → página sin girar.
  const tiles = createMemo(() => {
    const b = box();
    const vis = {
      left: 0,
      right: b.width,
      top: Math.max(0, props.scrollTop - 200 - top()),
      bottom: Math.min(b.height, props.scrollTop + props.viewport.height + 200 - top()),
    };
    if (vis.bottom <= vis.top) return [];
    return tilesFor(unrotated(), unrotateRect(vis, unrotated(), props.rotation));
  });
  const low = () => q(lowResZoom(raw()));

  return (
    <div class="page" style={{ top: `${top()}px`, width: `${box().width}px`, height: `${box().height}px` }} data-page={props.index}>
      <div class="page-rot" style={{ width: `${unrotated().width}px`, height: `${unrotated().height}px`, transform: rotationTransform(unrotated(), props.rotation) }}>
        {/* Render progresivo: primero la página entera a baja resolución, estirada. */}
        <img class="tile low" alt="" draggable={false} src={tileUrl(props.doc.id, props.index, low(), 0, 0, props.doc.revision)}
          style={{ width: `${unrotated().width}px`, height: `${unrotated().height}px` }} />
        <For each={tiles()}>{(t) => (
          <img class="tile" alt="" draggable={false}
            src={tileUrl(props.doc.id, props.index, q(props.zoom), t.col, t.row, props.doc.revision)}
            style={{ left: `${t.col * TILE_SIZE}px`, top: `${t.row * TILE_SIZE}px` }} />
        )}</For>
        <For each={matchesOnPage(props.index)}>{({ match, index }) => (
          <div class="hit" classList={{ current: index === matchIndex() }}
            style={{ left: `${match.rect.left * props.zoom}px`, top: `${match.rect.top * props.zoom}px`,
              width: `${(match.rect.right - match.rect.left) * props.zoom}px`, height: `${(match.rect.bottom - match.rect.top) * props.zoom}px` }} />
        )}</For>
        <For each={pageOverlays()}>{(Overlay) => <Overlay doc={props.doc} index={props.index} zoom={props.zoom} />}</For>
      </div>
    </div>
  );
}
