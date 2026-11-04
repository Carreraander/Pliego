import { hexToRgb, intIn, numIn } from "./util";

/** Construye los `StampOp` que entiende `stamp_pdf` (ver `pliego_stamp::StampOp`). Una entrada no válida usa el valor por defecto. */
export interface TextStyleIn { font: string; size: string; color: string }
const style = (s: TextStyleIn, def: number) => ({ font: s.font, size: intIn(s.size, 4, 400) ?? def, color: hexToRgb(s.color) });

export function watermarkOp(a: {
  kind: "text" | "image"; text: string; style: TextStyleIn; image: string | null; scalePct: number;
  opacityPct: number; rotation: string; behind: boolean;
}) {
  return {
    op: "watermark" as const,
    content: a.kind === "text"
      ? { kind: "text" as const, text: a.text, style: style(a.style, 72) }
      : { kind: "image" as const, path: a.image, scale: a.scalePct / 100 },
    opacity: a.opacityPct / 100,
    rotation: a.kind === "text" ? numIn(a.rotation, -360, 360) ?? 0 : 0,
    behind: a.behind,
  };
}

export function headerFooterOp(a: { header: [string, string, string]; footer: [string, string, string]; style: TextStyleIn; margin: string }) {
  const slots = ([left, center, right]: [string, string, string]) => ({ left, center, right });
  return { op: "header_footer" as const, header: slots(a.header), footer: slots(a.footer), style: style(a.style, 10), margin: intIn(a.margin, 0, 300) ?? 36 };
}

export function backgroundOp(a: { kind: "color" | "image"; color: string; image: string | null; opacityPct: number }) {
  return {
    op: "background" as const,
    fill: a.kind === "color" ? { kind: "color" as const, rgb: hexToRgb(a.color) } : { kind: "image" as const, path: a.image },
    opacity: a.opacityPct / 100,
  };
}

export function batesOp(a: { prefix: string; suffix: string; digits: string; start: string; corner: string; style: TextStyleIn }) {
  return {
    op: "bates" as const, prefix: a.prefix, suffix: a.suffix,
    digits: intIn(a.digits, 1, 12) ?? 6, start: intIn(a.start, 0, 999_999_999) ?? 1,
    corner: a.corner.replace("-", "_"), style: style(a.style, 10),
  };
}
