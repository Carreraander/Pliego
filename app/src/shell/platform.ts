/** Detección del entorno: la interfaz también se ejecuta en un navegador (desarrollo, pruebas de Playwright). */
export const isTauri = (): boolean => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export const isWindows = (): boolean => typeof navigator !== "undefined" && /windows/i.test(navigator.userAgent);

/** Windows 11 o posterior (Mica disponible). `userAgentData.platformVersion` ≥ 13 es Windows 11. */
export async function isWindows11(): Promise<boolean> {
  try {
    const uad = (navigator as Navigator & { userAgentData?: { getHighEntropyValues(h: string[]): Promise<{ platformVersion?: string }> } }).userAgentData;
    const v = (await uad?.getHighEntropyValues(["platformVersion"]))?.platformVersion;
    return v ? parseInt(v.split(".")[0] ?? "0", 10) >= 13 : false;
  } catch { return false; }
}

type WindowApi = typeof import("@tauri-apps/api/window");
let windowApi: Promise<WindowApi> | null = null;
/** Carga perezosa de la API de ventana de Tauri; `null` fuera de Tauri. */
export function tauriWindow() {
  if (!isTauri()) return null;
  windowApi ??= import("@tauri-apps/api/window");
  return windowApi.then((m) => m.getCurrentWindow());
}
