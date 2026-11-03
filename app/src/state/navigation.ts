import { createSignal } from "solid-js";

/** Petición de desplazamiento: el visor la atiende y la limpia. `y` es un desplazamiento opcional en puntos PDF. */
export interface GoTo {
  page: number;
  y?: number;
  /** Cambia en cada petición para que repetir el mismo destino vuelva a desplazar. */
  nonce: number;
}

const [request, setRequest] = createSignal<GoTo | null>(null);
let counter = 0;

export const goToRequest = request;

export function goToPage(page: number, y?: number): void {
  setRequest({ page, y, nonce: ++counter });
}

export function clearGoTo(): void {
  setRequest(null);
}
