import { createMemo, For } from "solid-js";
import { tileUrl } from "../../api";
import type { OpenDoc } from "../../state/documents";
import { TILE_SIZE, tilesFor } from "../../viewer/layout";
import type { Box } from "./geometry";

/** Mismo redondeo del zoom que el visor, para reutilizar las teselas ya cargadas. */
const q = (z: number) => Math.round(z * 1000) / 1000;

/** Escala y desplazamiento que llevan `from` (lo que se ve en la página) a llenar `to` (la caja que se arrastra). */
export function ghostTransform(from: Box, to: Box, zoom: number): string {
  const sx = (to[2] - to[0]) / Math.max(0.5, from[2] - from[0]);
  const sy = (to[3] - to[1]) / Math.max(0.5, from[3] - from[1]);
  return `scale(${sx}, ${sy}) translate(${-from[0] * zoom}px, ${-from[1] * zoom}px)`;
}

/**
 * Vista previa mientras se arrastra un objeto: recorta de la página ya dibujada (sus teselas) lo que hay dentro de la
 * caja original y lo estira a la caja nueva, de modo que se ve la imagen o el trazo moviéndose o cambiando de tamaño.
 * Se coloca dentro de la caja que se arrastra; no captura el ratón.
 */
export function Ghost(p: { doc: OpenDoc; index: number; zoom: number; from: Box; to: Box }) {
  const size = () => p.doc.sizes[p.index];
  const tiles = createMemo(() => {
    const s = size();
    if (!s) return [];
    const z = p.zoom;
    return tilesFor({ width: s.width * z, height: s.height * z }, {
      left: p.from[0] * z, top: p.from[1] * z, right: p.from[2] * z, bottom: p.from[3] * z,
    });
  });
  return (
    <div class="pl-edit-ghost" data-testid="edit-ghost" aria-hidden="true">
      <div class="pl-edit-ghost__page" style={{
        width: `${(size()?.width ?? 0) * p.zoom}px`, height: `${(size()?.height ?? 0) * p.zoom}px`,
        transform: ghostTransform(p.from, p.to, p.zoom),
      }}>
        <For each={tiles()}>{(t) => (
          <img alt="" draggable={false} src={tileUrl(p.doc.id, p.index, q(p.zoom), t.col, t.row, p.doc.revision)}
            style={{ left: `${t.col * TILE_SIZE}px`, top: `${t.row * TILE_SIZE}px` }} />
        )}</For>
      </div>
    </div>
  );
}
