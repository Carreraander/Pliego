import { withPassword } from "../../state/passwords";
import "./checks.css";
import { createEffect, createSignal, For, on, Show } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { activeDoc, currentPageIndex } from "../../state/documents";
import { Notice, TaskPanel } from "../../ui";
import { errorText } from "../run";
import { ct } from "./text";
import { overLimit, percent, plateSrc, spotInks, TAC_LIMIT, type OutputPreview } from "./output";

/** Resolución de la vista previa: suficiente para ver el reparto de tinta sin renderizar a tamaño de imprenta. */
const DPI = 40;

/** Vista previa de salida: las cuatro planchas CMYK de la página actual, la tinta total y las tintas directas. */
export default function OutputPanel(p: { onClose: () => void }) {
  const [data, setData] = createSignal<OutputPreview | null>(null);
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  const load = async () => {
    const doc = activeDoc();
    setData(null); setError(null);
    if (!doc) return;
    setBusy(true);
    try { setData(await invoke<OutputPreview>("output_preview", withPassword({ path: doc.path, page: currentPageIndex(), dpi: DPI }))); }
    catch (e) { setError(ct("checks.failed", { error: errorText(e) })); }
    finally { setBusy(false); }
  };
  createEffect(on([() => activeDoc()?.id, currentPageIndex], () => void load()));

  return (
    <TaskPanel
      title={ct("checks.output.title")} onClose={p.onClose} estimate={ct("checks.output.hint")}
      primary={{ label: busy() ? ct("checks.running") : ct("checks.output.refresh"), icon: "refresh", disabled: busy() || !activeDoc(), onClick: () => void load() }}
      notice={<Show when={error()}><Notice tone="danger" inline>{error()}</Notice></Show>}
    >
      <Show when={data()}>
        {(d) => (
          <>
            <Notice tone={overLimit(d().max_total_ink) ? "warn" : "ok"} inline>
              {ct(overLimit(d().max_total_ink) ? "checks.output.tacOver" : "checks.output.tac", { tac: percent(d().max_total_ink), limit: percent(TAC_LIMIT), page: currentPageIndex() + 1 })}
            </Notice>
            <div class="pl-plates">
              <For each={d().plates}>{(pl) => (
                <figure class="pl-plate">
                  <img src={plateSrc(pl)} alt={ct(`checks.output.${pl.channel}`)} />
                  <figcaption>{ct(`checks.output.${pl.channel}`)} · {percent(pl.coverage)} %</figcaption>
                </figure>
              )}</For>
            </div>
            <Show when={spotInks(d().inks).length}>
              <p>{ct("checks.output.spots", { names: spotInks(d().inks).map((i) => i.name).join(", ") })}</p>
            </Show>
          </>
        )}
      </Show>
    </TaskPanel>
  );
}
