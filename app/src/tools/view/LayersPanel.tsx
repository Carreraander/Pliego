import { createEffect, createSignal, For, on, Show } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { activeDoc } from "../../state/documents";
import { Checkbox, Notice, TaskPanel } from "../../ui";
import { errorText } from "../run";
import { vt } from "./strings";

interface Layer { id: number; name: string; visible: boolean }

/** Capas del documento (contenido opcional): mostrar u ocultar solo en pantalla; el archivo no cambia. */
export default function LayersPanel(p: { onClose: () => void }) {
  const [layers, setLayers] = createSignal<Layer[] | null>(null);
  const [error, setError] = createSignal<string | null>(null);

  const load = async () => {
    const doc = activeDoc();
    setError(null);
    if (!doc) { setLayers(null); return; }
    try { setLayers(await invoke<Layer[]>("layers", { docId: doc.id })); }
    catch (e) { setError(vt("view.layers.failed", { error: errorText(e) })); }
  };
  createEffect(on(() => activeDoc()?.id, () => { setLayers(null); void load(); }));

  const set = async (layer: Layer, visible: boolean) => {
    const doc = activeDoc();
    if (!doc) return;
    try { await invoke("set_layer_visible", { docId: doc.id, layer: layer.id, visible }); }
    catch (e) { setError(vt("view.layers.failed", { error: errorText(e) })); }
    await load();
  };
  const showAll = async () => {
    for (const l of layers() ?? []) if (!l.visible) await set(l, true);
  };

  return (
    <TaskPanel
      title={vt("view.layers.title")} onClose={p.onClose} estimate={vt("view.layers.hint")}
      primary={{ label: vt("view.layers.showAll"), icon: "layers", disabled: !layers()?.some((l) => !l.visible), onClick: () => void showAll() }}
      notice={<Show when={!activeDoc() || error()}><Notice tone={error() ? "danger" : "warn"} inline>{error() ?? vt("view.layers.noDoc")}</Notice></Show>}
    >
      <Show when={layers()}>
        {(list) => (
          <Show when={list().length} fallback={<p>{vt("view.layers.none")}</p>}>
            <For each={list()}>{(l) => <Checkbox label={l.name} checked={l.visible} onChange={(e) => void set(l, e.currentTarget.checked)} />}</For>
          </Show>
        )}
      </Show>
    </TaskPanel>
  );
}
