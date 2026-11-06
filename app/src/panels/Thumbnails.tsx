import { createSignal, For, onCleanup, onMount, untrack } from "solid-js";
import { tileUrl } from "../api";
import { goToPage } from "../state/navigation";
import { currentPageIndex, type OpenDoc } from "../state/documents";

const THUMB_WIDTH = 120;

/**
 * Las miniaturas solo existen en el DOM mientras están cerca de la zona visible del panel: con 500 páginas, dejar
 * todas cargadas mantendría en WebView2 cientos de imágenes decodificadas (memoria del renderizador). Fuera de esa
 * zona queda un hueco del mismo tamaño, así que la barra de desplazamiento no cambia.
 */
const NEAR = "600px 0px";

export function Thumbnails(props: { doc: OpenDoc }) {
  return (
    <div class="thumbs" data-testid="thumbnails">
      <For each={props.doc.sizes}>{(size, i) => {
        const zoom = Math.round((THUMB_WIDTH / size.width) * 1000) / 1000;
        const height = Math.round(size.height * zoom);
        const [near, setNear] = createSignal(typeof IntersectionObserver === "undefined" || untrack(i) < 8);
        let button!: HTMLButtonElement;
        onMount(() => {
          if (typeof IntersectionObserver === "undefined") return;
          const io = new IntersectionObserver((entries) => {
            for (const e of entries) setNear(e.isIntersecting);
          }, { rootMargin: NEAR });
          io.observe(button);
          onCleanup(() => io.disconnect());
        });
        return (
          <button ref={button} class="thumb" classList={{ active: currentPageIndex() === i() }} onClick={() => goToPage(i())} aria-label={`${i() + 1}`}>
            {near() ? (
              <img alt="" draggable={false} width={THUMB_WIDTH} height={height}
                src={tileUrl(props.doc.id, i(), zoom, 0, 0, props.doc.revision)} />
            ) : (
              <div class="thumb-gap" style={{ width: `${THUMB_WIDTH}px`, height: `${height}px` }} />
            )}
            <span>{i() + 1}</span>
          </button>
        );
      }}</For>
    </div>
  );
}
