/**
 * La app corre dentro de WebView2, que trae los atajos y el menú contextual de Edge. Sin este guardián, F5 o Ctrl+R
 * recargan la interfaz (se cierran todos los documentos), Ctrl+P imprime la ventana de la app, Ctrl+F abre la búsqueda
 * de Edge, Alt+← navega «atrás» y el clic derecho ofrece «Actualizar», «Guardar como» o «Imprimir».
 *
 * Se instala **después** del gestor de atajos (`navigation/shortcuts.ts`): las órdenes propias ya han hecho
 * `preventDefault` y aquí solo se para lo que el navegador haría por su cuenta.
 */

type KeyLike = Pick<KeyboardEvent, "key" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey">;

/** Teclas sueltas (sin Ctrl) que el navegador usa: recargar, buscar siguiente, navegación con el cursor, herramientas. */
const PLAIN = new Set(["F3", "F5", "F7", "F12", "BrowserBack", "BrowserForward", "BrowserRefresh", "BrowserSearch", "BrowserHome"]);
/** Letras que con Ctrl (con o sin Mayús) son del navegador, nunca de edición de texto. */
const CTRL_LETTERS = new Set(["D", "E", "F", "G", "H", "J", "L", "N", "O", "P", "R", "S", "T", "U", "W"]);
/** Con Ctrl+Mayús, además: herramientas de desarrollo, borrar datos, nueva ventana privada, favoritos… */
const CTRL_SHIFT_LETTERS = new Set(["B", "C", "I", "J", "K", "M", "N", "O", "P", "R", "S", "T", "Y", "DELETE"]);

/** ¿Es un atajo del navegador que hay que parar? Pura, para probarla sin DOM. */
export function isBrowserShortcut(e: KeyLike, dev = false): boolean {
  const key = e.key.length === 1 ? e.key.toUpperCase() : e.key;
  // En desarrollo se dejan las herramientas de desarrollo (F12, Ctrl+Mayús+I/J/C).
  if (dev && (key === "F12" || (e.ctrlKey && e.shiftKey && ["I", "J", "C"].includes(key)))) return false;
  if (!e.ctrlKey && !e.altKey && !e.metaKey && PLAIN.has(key)) return true;
  if (e.altKey && !e.ctrlKey && (key === "ArrowLeft" || key === "ArrowRight" || key === "Home")) return true;
  if (!e.ctrlKey || e.altKey) return false;
  if (key === "F5" || key === "F4") return true;
  // Zoom de la página (Tauri ya lo desactiva, pero no cuesta asegurarlo) y la rueda se trata aparte.
  if (["+", "-", "=", "_", "0"].includes(key)) return true;
  if (e.shiftKey) return CTRL_SHIFT_LETTERS.has(key.toUpperCase());
  // Ctrl+A/C/V/X/Z/Y no están en la lista: la edición de texto sigue funcionando en los campos.
  return CTRL_LETTERS.has(key);
}

const editable = (el: EventTarget | null): boolean => {
  const n = el as HTMLElement | null;
  return !!n && (n.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(n.tagName));
};

/** Instala el guardián. Devuelve cómo quitarlo. */
export function installBrowserGuard(): () => void {
  const onKey = (e: KeyboardEvent) => {
    if (e.defaultPrevented) return;
    if (isBrowserShortcut(e, import.meta.env.DEV)) e.preventDefault();
  };
  // El menú de Edge solo se deja en los campos de texto (cortar, copiar, pegar). Los menús propios ya hacen preventDefault.
  const onContext = (e: MouseEvent) => {
    if (e.defaultPrevented || editable(e.target)) return;
    e.preventDefault();
  };
  // Ctrl+rueda ampliaría la interfaz entera.
  const onWheel = (e: WheelEvent) => { if (e.ctrlKey) e.preventDefault(); };
  // Botones laterales del ratón: «atrás»/«adelante» del navegador.
  const onMouse = (e: MouseEvent) => { if (e.button === 3 || e.button === 4) e.preventDefault(); };
  addEventListener("keydown", onKey);
  addEventListener("contextmenu", onContext);
  addEventListener("wheel", onWheel, { passive: false });
  addEventListener("mouseup", onMouse);
  return () => {
    removeEventListener("keydown", onKey);
    removeEventListener("contextmenu", onContext);
    removeEventListener("wheel", onWheel);
    removeEventListener("mouseup", onMouse);
  };
}
