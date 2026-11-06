/**
 * Órdenes de archivo y del visor que la hoja de atajos (§6, `navigation/reference.ts`) ya prometía: abrir, cerrar y
 * reabrir pestañas, cambiar de pestaña, zoom, girar, ir a la primera/última/anterior/siguiente página y buscar.
 * Se registran con el mismo `id` que la fila de referencia, así que sustituyen a la fila «solo listada».
 */
import { locale } from "../i18n";
import { registerShortcut, type ShortcutDef } from "../navigation/shortcuts";
import { activeMode, setMode } from "../shell/slots";
import {
  activeDoc, activeId, closeDoc, currentPageIndex, docs, popClosedPath, rotation, setActiveId, setRotation, setZoom, zoom,
} from "../state/documents";
import { goToPage } from "../state/navigation";
import { matches, nextMatch } from "../state/search";
import { toast } from "../ui/toastStore";
import { fitPageZoom, fitWidthZoom, rotate, stepZoom, type Size } from "./layout";

export interface ViewerCommandHost {
  /** Abre un archivo (con el diálogo de contraseña si hace falta). */
  open: (path: string) => void;
  /** Muestra el selector de archivos y abre lo elegido. */
  pickAndOpen: () => void;
  /** Tamaño del lienzo, para los zooms de ajuste. */
  viewport: () => Size;
  /** Cierra la pestaña activa si es una pantalla (Inicio, Ajustes); `true` si lo hizo. */
  closeActiveScreen: () => boolean;
}

/** Siguiente id de documento al recorrer las pestañas (con vuelta). Pura, para probarla. */
export function cycleTab(ids: number[], current: number | null, dir: 1 | -1): number | null {
  if (!ids.length) return null;
  const i = current === null ? -1 : ids.indexOf(current);
  if (i < 0) return ids[dir === 1 ? 0 : ids.length - 1]!;
  return ids[(i + dir + ids.length) % ids.length]!;
}

/** Página de destino de las órdenes de paso de página. Pura, para probarla. */
export function targetPage(order: "first" | "last" | "prev" | "next", current: number, total: number): number {
  if (total <= 0) return 0;
  const last = total - 1;
  switch (order) {
    case "first": return 0;
    case "last": return last;
    case "prev": return Math.max(0, Math.min(last, current - 1));
    case "next": return Math.max(0, Math.min(last, current + 1));
  }
}

/** Las teclas de página solo se usan en el visor de Leer, para no quitar Inicio/Fin a listas y rejillas. */
function viewerHasFocus(): boolean {
  if (activeMode()?.id !== "read" || !activeDoc()) return false;
  const el = document.activeElement as HTMLElement | null;
  return !el || el === document.body || !!el.closest('[data-region="canvas"]');
}

const es = () => locale() === "es";

export function installViewerCommands(host: ViewerCommandHost): () => void {
  const hasDoc = () => activeDoc() !== null;
  const first = () => activeDoc()?.sizes[currentPageIndex()] ?? activeDoc()?.sizes[0];
  const page = (order: "first" | "last" | "prev" | "next") => () => {
    const d = activeDoc();
    if (d) goToPage(targetPage(order, currentPageIndex(), d.sizes.length));
  };
  const tab = (dir: 1 | -1) => () => {
    const next = cycleTab(docs().map((d) => d.id), activeId(), dir);
    if (next !== null) setActiveId(next);
  };
  const defs: ShortcutDef[] = [
    { id: "file.open", section: "file", keys: "Ctrl+O", title: () => (es() ? "Abrir" : "Open"), run: host.pickAndOpen },
    {
      id: "file.closeTab", section: "file", keys: "Ctrl+W", title: () => (es() ? "Cerrar pestaña" : "Close tab"),
      run: () => { if (host.closeActiveScreen()) return; const id = activeId(); if (id !== null) void closeDoc(id); },
    },
    {
      id: "file.reopenTab", section: "file", keys: "Ctrl+Shift+T", title: () => (es() ? "Reabrir pestaña cerrada" : "Reopen closed tab"),
      run: () => { const p = popClosedPath(); if (p) host.open(p); },
    },
    {
      // Hasta que llegue la impresión (docs/handoff/huecos-1.0.md, P0 #1), Ctrl+P no debe imprimir la interfaz.
      id: "file.print", section: "file", keys: "Ctrl+P", title: () => (es() ? "Imprimir" : "Print"),
      run: () => { toast({ tone: "info", text: es() ? "La impresión llegará en una próxima versión." : "Printing is coming in a future version." }); },
    },
    { id: "nav.nextTab", section: "navigation", keys: "Ctrl+Tab", title: () => (es() ? "Pestaña siguiente" : "Next tab"), run: tab(1) },
    { id: "nav.prevTab", section: "navigation", keys: "Ctrl+Shift+Tab", title: () => (es() ? "Pestaña anterior" : "Previous tab"), run: tab(-1) },
    { id: "nav.home", section: "navigation", keys: "Home", scope: "canvas", title: () => (es() ? "Primera página" : "First page"), run: page("first"), when: viewerHasFocus },
    { id: "nav.end", section: "navigation", keys: "End", scope: "canvas", title: () => (es() ? "Última página" : "Last page"), run: page("last"), when: viewerHasFocus },
    { id: "nav.pageUp", section: "navigation", keys: "PageUp", scope: "canvas", title: () => (es() ? "Página anterior" : "Previous page"), run: page("prev"), when: viewerHasFocus },
    { id: "nav.pageDown", section: "navigation", keys: "PageDown", scope: "canvas", title: () => (es() ? "Página siguiente" : "Next page"), run: page("next"), when: viewerHasFocus },
    { id: "zoom.in", section: "zoom", keys: "Ctrl++", title: () => (es() ? "Acercar" : "Zoom in"), run: () => setZoom(stepZoom(zoom(), 1)), when: hasDoc },
    { id: "zoom.out", section: "zoom", keys: "Ctrl+-", title: () => (es() ? "Alejar" : "Zoom out"), run: () => setZoom(stepZoom(zoom(), -1)), when: hasDoc },
    {
      id: "zoom.fitPage", section: "zoom", keys: "Ctrl+0", title: () => (es() ? "Ajustar a la página" : "Fit page"), when: hasDoc,
      run: () => { const p = first(); if (p) setZoom(fitPageZoom(p, rotation(), host.viewport())); },
    },
    { id: "zoom.actual", section: "zoom", keys: "Ctrl+1", title: () => (es() ? "Tamaño real (100 %)" : "Actual size (100%)"), run: () => setZoom(1), when: hasDoc },
    {
      id: "zoom.fitWidth", section: "zoom", keys: "Ctrl+2", title: () => (es() ? "Ajustar al ancho" : "Fit width"), when: hasDoc,
      run: () => { const p = first(); if (p) setZoom(fitWidthZoom(p, rotation(), host.viewport().width)); },
    },
    { id: "view.rotate", section: "zoom", keys: "Ctrl+R", title: () => (es() ? "Girar la vista" : "Rotate view"), run: () => setRotation(rotate(rotation())), when: hasDoc },
    {
      id: "search.find", section: "search", keys: "Ctrl+F", title: () => (es() ? "Buscar en el documento" : "Find in document"), when: hasDoc,
      run: () => {
        if (activeMode()?.id !== "read") setMode("read");
        // La caja de búsqueda está en la barra de Leer: tras cambiar de modo hay que esperar a que se monte.
        requestAnimationFrame(() => {
          const input = document.querySelector<HTMLInputElement>(".searchbox input");
          input?.focus();
          input?.select();
        });
      },
    },
    { id: "search.next", section: "search", keys: "F3", title: () => (es() ? "Siguiente coincidencia" : "Next match"), run: () => nextMatch(1), when: () => matches().length > 0 },
    { id: "search.prev", section: "search", keys: "Shift+F3", title: () => (es() ? "Coincidencia anterior" : "Previous match"), run: () => nextMatch(-1), when: () => matches().length > 0 },
  ];
  const offs = defs.map(registerShortcut);

  // Variantes del zoom según el teclado (Ctrl+= en el inglés de EE. UU., Ctrl+Mayús++) y Ctrl+rueda sobre el visor.
  const onKey = (e: KeyboardEvent) => {
    if (e.defaultPrevented || !e.ctrlKey || e.altKey || !hasDoc()) return;
    const dir = zoomKeyDirection(e.key);
    if (dir) { e.preventDefault(); setZoom(stepZoom(zoom(), dir)); }
  };
  const onWheel = (e: WheelEvent) => {
    if (!e.ctrlKey || !hasDoc() || !(e.target as HTMLElement | null)?.closest?.(".viewer")) return;
    e.preventDefault();
    if (e.deltaY) setZoom(stepZoom(zoom(), e.deltaY < 0 ? 1 : -1));
  };
  addEventListener("keydown", onKey);
  addEventListener("wheel", onWheel, { passive: false });
  return () => {
    offs.forEach((off) => off());
    removeEventListener("keydown", onKey);
    removeEventListener("wheel", onWheel);
  };
}

/** Teclas que, con Ctrl (y con o sin Mayús), acercan o alejan; Ctrl++ y Ctrl+- ya los trata la tabla de atajos. */
export function zoomKeyDirection(key: string): 1 | -1 | 0 {
  if (key === "=" || key === "+") return 1;
  if (key === "_" || key === "-") return -1;
  return 0;
}
