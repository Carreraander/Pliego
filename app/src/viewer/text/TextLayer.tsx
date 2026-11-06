import { createMemo, createResource, For, Show } from "solid-js";
import { t } from "../../i18n";
import type { OpenDoc } from "../../state/documents";
import { highlightRects, paragraphs, rangeOnPage } from "./model";
import { loadWords, selectionOf } from "./state";

/**
 * Capa de texto de una página del visor: pinta la selección y ofrece el texto a los lectores de pantalla. Las páginas son
 * imágenes, así que el texto sale de las palabras con caja que da el worker. La selección con el ratón y el teclado la
 * gestiona el visor entero (`controller.ts`); esta capa no recoge eventos.
 */
export function TextLayer(props: { doc: OpenDoc; index: number; zoom: number }) {
  const [words] = createResource(
    () => ({ id: props.doc.id, revision: props.doc.revision, page: props.index }),
    (k) => loadWords(k, k.page),
  );
  const rects = createMemo(() => {
    const w = words();
    const sel = selectionOf(props.doc);
    const range = w && sel ? rangeOnPage(sel, props.index, w.length) : null;
    return w && range ? highlightRects(w, range[0], range[1]) : [];
  });
  const total = () => props.doc.sizes.length;
  const label = () => t("viewer.page", { page: props.index + 1, total: total() });

  return (
    <>
      <For each={rects()}>{(r) => (
        <div class="pl-sel" aria-hidden="true"
          style={{ left: `${r[0] * props.zoom}px`, top: `${r[1] * props.zoom}px`, width: `${(r[2] - r[0]) * props.zoom}px`, height: `${(r[3] - r[1]) * props.zoom}px` }} />
      )}</For>
      {/* Texto de la página para el Narrador y otros lectores: fuera de la vista, en orden de lectura. */}
      <div class="pl-sr-only" role="group" aria-label={label()} data-testid="page-text">
        <Show when={words()} fallback={<p>{label()}</p>}>
          {(w) => (
            <Show when={w().length > 0} fallback={<p>{t("viewer.pageNoText", { page: props.index + 1, total: total() })}</p>}>
              <For each={paragraphs(w())}>{(p) => <p>{p}</p>}</For>
            </Show>
          )}
        </Show>
      </div>
    </>
  );
}
