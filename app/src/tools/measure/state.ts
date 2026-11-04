import { createSignal } from "solid-js";
import type { MeasureMode, Pt, Scale } from "./geometry";

/** El panel «Medir» está abierto: solo entonces la capa del visor recoge clics. */
export const [measureOpen, setMeasureOpen] = createSignal(false);
export const [measureMode, setMeasureMode] = createSignal<MeasureMode>("distance");
/** Marcas en curso y la página (desde 0) a la que pertenecen. */
export const [marks, setMarks] = createSignal<{ page: number; points: Pt[] }>({ page: 0, points: [] });
/** Escala activa del documento (la de la página si la tiene, o la que acaba de calibrarse). */
export const [activeScale, setActiveScale] = createSignal<Scale | null>(null);
