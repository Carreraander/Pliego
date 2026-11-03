/** Combinaciones de teclas: normalización, comparación con eventos y texto para mostrar. */

const ALIASES: Record<string, string> = {
  control: "Ctrl", ctrl: "Ctrl", alt: "Alt", shift: "Shift", "mayús": "Shift", mayus: "Shift", meta: "Meta",
  esc: "Escape", escape: "Escape", supr: "Delete", del: "Delete", delete: "Delete", " ": "Space", space: "Space", espacio: "Space",
  inicio: "Home", fin: "End", repág: "PageUp", repag: "PageUp", avpág: "PageDown", avpag: "PageDown",
  "←": "ArrowLeft", "→": "ArrowRight", "↑": "ArrowUp", "↓": "ArrowDown", intro: "Enter",
};
const ORDER = ["Ctrl", "Alt", "Shift", "Meta"];

/** `"mayús+ctrl+a"` → `"Ctrl+Shift+A"` (orden fijo; tecla en mayúscula si es una letra). */
export function normalizeCombo(combo: string): string {
  const parts = combo.split("+").map((p) => p.trim()).filter(Boolean);
  if (combo.trim().endsWith("++")) parts.push("+");
  const mods = new Set<string>();
  let key = "";
  for (const p of parts) {
    const name = ALIASES[p.toLowerCase()] ?? p;
    if (ORDER.includes(name)) mods.add(name);
    else key = name.length === 1 ? name.toUpperCase() : name.replace(/^f(\d+)$/i, "F$1");
  }
  return [...ORDER.filter((m) => mods.has(m)), key].filter(Boolean).join("+");
}

/** Combinación de un evento de teclado, ya normalizada; `null` si solo es un modificador. */
export function eventCombo(e: Pick<KeyboardEvent, "key" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey">): string | null {
  if (["Control", "Alt", "Shift", "Meta", "AltGraph"].includes(e.key)) return null;
  const mods = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Meta"].filter(Boolean) as string[];
  const key = e.key === " " ? "Space" : e.key.length === 1 ? e.key.toUpperCase() : e.key;
  return [...mods, key].join("+");
}

/** Texto para mostrar: `Ctrl+Shift+A` → `Ctrl+Mayús+A`. */
export function displayCombo(combo: string, loc: "es" | "en"): string {
  if (!combo) return "";
  return combo.split("+").map((p) => {
    if (loc === "es") return ({ Shift: "Mayús", Delete: "Supr", Escape: "Esc", PageUp: "RePág", PageDown: "AvPág", Home: "Inicio", End: "Fin", ArrowLeft: "←", ArrowRight: "→", ArrowUp: "↑", ArrowDown: "↓", Space: "Espacio" } as Record<string, string>)[p] ?? p;
    return ({ Escape: "Esc", Delete: "Del", ArrowLeft: "←", ArrowRight: "→", ArrowUp: "↑", ArrowDown: "↓" } as Record<string, string>)[p] ?? p;
  }).join("+");
}

/** ¿Es una tecla sin modificadores que escribe un carácter? (no debe activarse al escribir en un campo). */
export function isPlainKey(combo: string): boolean {
  const parts = combo.split("+");
  return parts.length === 1 && (parts[0]!.length === 1);
}
