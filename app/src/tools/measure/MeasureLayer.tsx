import "./measure.css";
import { For, Show } from "solid-js";
import type { PageOverlay } from "../../viewer/overlays";
import { addPoint, type Pt, svgPoints } from "./geometry";
import { marks, measureMode, measureOpen, setMarks } from "./state";

/** Capa de una página en el modo Medir: cada clic marca un punto y se dibuja la figura en curso. */
export const MeasureLayer: PageOverlay = (p) => {
  const here = (): Pt[] => (marks().page === p.index ? marks().points : []);
  const click = (e: MouseEvent) => {
    if (e.button !== 0) return;
    const r = e.currentTarget as HTMLElement;
    const box = r.getBoundingClientRect();
    const pt: Pt = [(e.clientX - box.left) / p.zoom, (e.clientY - box.top) / p.zoom];
    setMarks((m) => ({ page: p.index, points: addPoint(m.page === p.index ? m.points : [], pt, measureMode()) }));
  };
  return (
    <Show when={measureOpen()}>
      <div class="pl-measure-layer" data-testid="measure-layer" onClick={click}>
        <svg aria-hidden="true">
          <Show when={here().length > 1}>
            {measureMode() === "area" && here().length > 2
              ? <polygon class="pl-measure-shape" points={svgPoints(here(), p.zoom)} />
              : <polyline class="pl-measure-shape" data-open="true" points={svgPoints(here(), p.zoom)} />}
          </Show>
          <For each={here()}>{(pt) => <circle class="pl-measure-dot" cx={pt[0] * p.zoom} cy={pt[1] * p.zoom} r={4} />}</For>
        </svg>
      </div>
    </Show>
  );
};
