/** Documentos con cambios sin guardar: quien los edite se registra aquí y "Reiniciar ahora" los pide guardar. */
import { createSignal } from "solid-js";

export interface UnsavedDoc { title: string; save: () => Promise<boolean> }
type Provider = () => UnsavedDoc[];

const [providers, setProviders] = createSignal<Provider[]>([]);
export function registerUnsavedProvider(p: Provider): () => void {
  setProviders((all) => [...all, p]);
  return () => setProviders((all) => all.filter((x) => x !== p));
}
export const unsavedDocs = (): UnsavedDoc[] => providers().flatMap((p) => p());
