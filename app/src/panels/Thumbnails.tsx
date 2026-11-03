import { For } from "solid-js";
import { tileUrl } from "../api";
import { goToPage } from "../state/navigation";
import { currentPageIndex, type OpenDoc } from "../state/documents";

const THUMB_WIDTH = 120;

export function Thumbnails(props: { doc: OpenDoc }) {
  return (
    <div class="thumbs" data-testid="thumbnails">
      <For each={props.doc.sizes}>{(size, i) => {
        const zoom = Math.round((THUMB_WIDTH / size.width) * 1000) / 1000;
        return (
          <button class="thumb" classList={{ active: currentPageIndex() === i() }} onClick={() => goToPage(i())} aria-label={`${i() + 1}`}>
            <img loading="lazy" alt="" draggable={false} width={THUMB_WIDTH} height={Math.round(size.height * zoom)}
              src={tileUrl(props.doc.id, i(), zoom, 0, 0, props.doc.revision)} />
            <span>{i() + 1}</span>
          </button>
        );
      }}</For>
    </div>
  );
}
