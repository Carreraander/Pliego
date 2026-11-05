import { createSignal } from "solid-js";
import { tauriWindow } from "../../shell/platform";
import { activeDoc, currentPageIndex, rotation, setRotation, setZoom, zoom } from "../../state/documents";
import { goToPage } from "../../state/navigation";
import { fitPageZoom, PAGE_GAP } from "../../viewer/layout";

/** `goToPage` deja la página 8 px bajo el borde superior y la siguiente empieza `PAGE_GAP` px tras la actual: con 14 px de margen la página cabe entera y la siguiente queda fuera (con 2 × `PAGE_GAP` asomaban ~4 px). */
const presentMargin = (): number => 8 + PAGE_GAP - 6;

/** `true` mientras se presenta: la ventana está a pantalla completa y sin armazón. */
export const [presenting, setPresenting] = createSignal(false);

/** Página a la que lleva una tecla de presentación (`null` si no es de navegación o no hay a dónde ir). */
export function targetPage(key: string, current: number, count: number): number | null {
  if (count <= 0) return null;
  const last = count - 1;
  let to: number;
  switch (key) {
    case "ArrowRight": case "PageDown": case " ": case "Enter": to = current + 1; break;
    case "ArrowLeft": case "PageUp": case "Backspace": to = current - 1; break;
    case "Home": to = 0; break;
    case "End": to = last; break;
    default: return null;
  }
  to = Math.min(last, Math.max(0, to));
  return to === current ? null : to;
}

let saved: { zoom: number; rotation: ReturnType<typeof rotation> } | null = null;
let onKey: ((e: KeyboardEvent) => void) | null = null;

const nextFrame = () => new Promise<void>((r) => requestAnimationFrame(() => requestAnimationFrame(() => r())));

async function setFullscreen(on: boolean): Promise<void> {
  const win = tauriWindow();
  try {
    if (win) await (await win).setFullscreen(on);
    else if (on) await document.documentElement.requestFullscreen?.();
    else if (document.fullscreenElement) await document.exitFullscreen();
  } catch { /* sin permiso o sin pantalla completa: la presentación sigue en ventana */ }
}

/** Ajusta el zoom para que la página actual quepa entera en el visor. */
function fitCurrent(): void {
  const doc = activeDoc();
  const view = document.querySelector<HTMLElement>('[data-testid="viewer"]');
  const size = doc?.sizes[currentPageIndex()];
  if (!doc || !view || !size) return;
  setZoom(fitPageZoom(size, rotation(), { width: view.clientWidth, height: view.clientHeight }, presentMargin()));
  goToPage(currentPageIndex());
}

export async function startPresentation(): Promise<void> {
  const doc = activeDoc();
  if (!doc || presenting()) return;
  saved = { zoom: zoom(), rotation: rotation() };
  document.documentElement.setAttribute("data-presenting", "");
  setPresenting(true);
  await setFullscreen(true);
  await nextFrame();
  fitCurrent();
  onKey = (e) => {
    if (e.key === "Escape" || e.key === "F11") { e.preventDefault(); e.stopPropagation(); void stopPresentation(); return; }
    const to = targetPage(e.key, currentPageIndex(), activeDoc()?.sizes.length ?? 0);
    if (to !== null) { e.preventDefault(); e.stopPropagation(); goToPage(to); }
  };
  window.addEventListener("keydown", onKey, true);
  window.addEventListener("resize", fitCurrent);
}

export async function stopPresentation(): Promise<void> {
  if (!presenting()) return;
  if (onKey) window.removeEventListener("keydown", onKey, true);
  window.removeEventListener("resize", fitCurrent);
  onKey = null;
  document.documentElement.removeAttribute("data-presenting");
  setPresenting(false);
  await setFullscreen(false);
  if (saved) { setZoom(saved.zoom); setRotation(saved.rotation); }
  saved = null;
}

/** F11: entra o sale. */
export const togglePresentation = (): Promise<void> => (presenting() ? stopPresentation() : startPresentation());
