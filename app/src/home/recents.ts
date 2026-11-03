import { invoke } from "@tauri-apps/api/core";
import { createSignal } from "solid-js";
import { isTauri } from "../shell/platform";

export interface Recent {
  path: string;
  name: string;
  folder: string;
  /** Segundos desde 1970. */
  opened_at: number;
  pinned: boolean;
  exists: boolean;
  size: number | null;
}

/** Dónde viven los recientes: el comando Tauri, o uno en memoria (desarrollo y pruebas). */
export interface RecentsBackend {
  list(): Promise<Recent[]>;
  add(path: string): Promise<void>;
  remove(path: string): Promise<void>;
  pin(path: string, pinned: boolean): Promise<void>;
  thumbnail(path: string): Promise<ArrayBuffer | null>;
  reveal(path: string): Promise<void>;
}

const tauriBackend: RecentsBackend = {
  list: () => invoke<Recent[]>("recents_list"),
  add: (path) => invoke("recents_add", { path }),
  remove: (path) => invoke("recents_remove", { path }),
  pin: (path, pinned) => invoke("recents_pin", { path, pinned }),
  thumbnail: async (path) => { try { return await invoke<ArrayBuffer>("recent_thumbnail", { path }); } catch { return null; } },
  reveal: (path) => invoke("reveal_path", { path }),
};

export function memoryBackend(initial: Recent[] = []): RecentsBackend {
  let items = [...initial];
  const sort = () => { items.sort((a, b) => Number(b.pinned) - Number(a.pinned) || b.opened_at - a.opened_at); };
  return {
    list: async () => [...items],
    add: async (path) => {
      const old = items.find((r) => r.path === path);
      items = items.filter((r) => r.path !== path);
      const name = path.split(/[\\/]/).pop() ?? path;
      items.push({ path, name, folder: path.slice(0, path.length - name.length - 1), opened_at: Math.floor(Date.now() / 1000), pinned: old?.pinned ?? false, exists: true, size: null });
      sort();
    },
    remove: async (path) => { items = items.filter((r) => r.path !== path); },
    pin: async (path, pinned) => { items = items.map((r) => (r.path === path ? { ...r, pinned } : r)); sort(); },
    thumbnail: async () => null,
    reveal: async () => undefined,
  };
}

let backend: RecentsBackend = isTauri() ? tauriBackend : memoryBackend();
export const setRecentsBackend = (b: RecentsBackend): void => { backend = b; };

const [recents, setRecents] = createSignal<Recent[]>([]);
export { recents };
export async function refreshRecents(): Promise<void> {
  try { setRecents(await backend.list()); } catch { setRecents([]); }
}
export async function addRecent(path: string): Promise<void> { try { await backend.add(path); } catch { /* sin recientes */ } await refreshRecents(); }
export async function removeRecent(path: string): Promise<void> { await backend.remove(path); await refreshRecents(); }
export async function pinRecent(path: string, pinned: boolean): Promise<void> { await backend.pin(path, pinned); await refreshRecents(); }
export const revealRecent = (path: string): Promise<void> => backend.reveal(path);

const thumbs = new Map<string, Promise<string | null>>();
/** URL (blob) de la miniatura de la primera página; `null` si no se pudo (contraseña, archivo dañado). Con caché. */
export function thumbnailFor(path: string): Promise<string | null> {
  let p = thumbs.get(path);
  if (!p) {
    p = backend.thumbnail(path).then((bytes) => (bytes ? URL.createObjectURL(new Blob([bytes], { type: "image/png" })) : null)).catch(() => null);
    thumbs.set(path, p);
  }
  return p;
}
