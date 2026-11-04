import { hexToRgb } from "./util";

export type Place = "center" | "top_left" | "top_right" | "bottom_left" | "bottom_right";
export const PLACES: Place[] = ["center", "top_left", "top_right", "bottom_left", "bottom_right"];
export const PRESETS = ["aprobado", "rechazado", "borrador", "confidencial", "revisado", "urgente", "copia", "final"] as const;
export type Preset = (typeof PRESETS)[number];

/** `StampRequest` de `pliego_app_service::stamps`. */
export function stampRequest(a: { kind: "preset" | "text" | "image"; preset: Preset; text: string; color: string; image: string | null; place: Place }) {
  const source =
    a.kind === "preset" ? { kind: "preset" as const, preset: a.preset }
    : a.kind === "text" ? { kind: "text" as const, text: a.text, color: hexToRgb(a.color) }
    : { kind: "image" as const, path: a.image };
  return { source, place: a.place };
}

/** Páginas que se envían: la actual (número desde 1) o todas. */
export const stampPages = (scope: "current" | "all", currentIndex: number): string => (scope === "all" ? "all" : String(currentIndex + 1));
