import { createSignal } from "solid-js";

/**
 * Mensajes para lectores de pantalla (Narrador): el visor tiene una región `aria-live` que lee lo que se escriba aquí.
 * Sirve para lo que no cambia el foco: la página a la que se llega al desplazarse, el texto que se va seleccionando con
 * Mayús+flechas o el lugar al que se vuelve con Alt+←.
 */
const [message, setMessage] = createSignal("");
export const announcement = message;

let clear: ReturnType<typeof setTimeout> | undefined;
let nonce = 0;

/** Anuncia `text`. Repetir el mismo texto lo vuelve a leer (se vacía la región un instante antes). */
export function announce(text: string): void {
  clearTimeout(clear);
  setMessage("");
  const mine = ++nonce;
  // Un cambio de contenido en la misma tarea no siempre se detecta: se escribe en la siguiente.
  setTimeout(() => {
    if (mine !== nonce) return;
    setMessage(text);
    clear = setTimeout(() => setMessage(""), 4000);
  }, 30);
}

/** Recorta un texto largo para leerlo en voz alta. */
export function brief(text: string, max = 160): string {
  const t = text.replace(/\s+/g, " ").trim();
  return t.length > max ? `${t.slice(0, max - 1)}…` : t;
}

/** Solo para pruebas. */
export function resetAnnouncements(): void { clearTimeout(clear); nonce++; setMessage(""); }
