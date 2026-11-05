import { withPassword } from "../../state/passwords";
import { createEffect, createSignal, For, Show } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { activeDoc } from "../../state/documents";
import { Button, Notice, TaskPanel } from "../../ui";
import { sizeLabel, type PortfolioEntry, type PortfolioListing } from "../batch/attachments";
import { errorText, fileName, runToolJob } from "../run";
import { vt } from "./strings";

/** Archivos adjuntos del documento: lista y extracción (todos o uno), sin modificar el PDF. */
export default function AttachmentsPanel(p: { onClose: () => void }) {
  const [entries, setEntries] = createSignal<PortfolioEntry[] | null>(null);
  const [error, setError] = createSignal<string | null>(null);

  createEffect(() => {
    const doc = activeDoc();
    setEntries(null); setError(null);
    if (!doc) return;
    invoke<PortfolioListing>("portfolio_list", withPassword({ path: doc.path }))
      .then((l) => setEntries(l.entries))
      .catch((e) => setError(vt("view.att.failed", { error: errorText(e) })));
  });

  const extract = async (names: string[]) => {
    const doc = activeDoc();
    if (!doc) return;
    const id = await runToolJob({
      command: "portfolio_extract", args: { path: doc.path, names, suffix: vt("view.att.suffix") },
      title: vt("view.att.job", { name: fileName(doc.path) }), started: vt("view.att.started"),
    });
    if (id !== null) p.onClose();
  };

  return (
    <TaskPanel
      title={vt("view.att.title")} onClose={p.onClose} estimate={vt("view.att.hint")}
      primary={{ label: vt("view.att.extractAll"), icon: "download", disabled: !entries()?.length, onClick: () => void extract([]) }}
      notice={<Show when={!activeDoc() || error()}><Notice tone={error() ? "danger" : "warn"} inline>{error() ?? vt("view.att.noDoc")}</Notice></Show>}
    >
      <Show when={entries()}>
        {(list) => (
          <Show when={list().length} fallback={<p>{vt("view.att.none")}</p>}>
            <ul>
              <For each={list()}>{(e) => (
                <li>
                  {e.filename} <small>({sizeLabel(e.size)})</small>{" "}
                  <Button onClick={() => void extract([e.name])}>{vt("view.att.extract")}</Button>
                </li>
              )}</For>
            </ul>
          </Show>
        )}
      </Show>
    </TaskPanel>
  );
}
