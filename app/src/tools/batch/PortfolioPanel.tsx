import { withPassword } from "../../state/passwords";
import "./batch.css";
import { createEffect, createSignal, For, Show } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { locale } from "../../i18n";
import { api } from "../../api";
import { activeDoc } from "../../state/documents";
import { Button, IconButton, Notice, TaskPanel } from "../../ui";
import { errorText, fileName, runToolJob } from "../run";
import { sizeLabel, type PortfolioListing } from "./attachments";
import { bt } from "./strings";

/** Carteras PDF: crear una con varios archivos y extraer los adjuntos del documento abierto. */
export default function PortfolioPanel(p: { onClose: () => void }) {
  const [files, setFiles] = createSignal<string[]>([]);
  const [listing, setListing] = createSignal<PortfolioListing | null>(null);
  const [error, setError] = createSignal<string | null>(null);

  createEffect(() => {
    const doc = activeDoc();
    setListing(null);
    if (!doc) return;
    invoke<PortfolioListing>("portfolio_list", withPassword({ path: doc.path }))
      .then(setListing)
      .catch((e) => setError(bt("batch.portfolio.listFailed", { error: errorText(e) })));
  });

  const add = async () => {
    const picked = await api.pickAnyFiles(bt("batch.portfolio.files"));
    if (picked.length) setFiles((f) => [...f, ...picked.filter((x) => !f.includes(x))]);
  };
  const create = async () => {
    const paths = files();
    const id = await runToolJob({
      command: "portfolio_create", args: { paths, suffix: bt("batch.portfolio.suffix"), lang: locale() },
      title: bt("batch.portfolio.job", { n: paths.length }), started: bt("batch.portfolio.started"),
    });
    if (id !== null) p.onClose();
  };
  const extract = async () => {
    const doc = activeDoc();
    if (!doc) return;
    const id = await runToolJob({
      command: "portfolio_extract", args: { path: doc.path, names: [], suffix: bt("batch.portfolio.extractSuffix") },
      title: bt("batch.portfolio.extractJob", { name: fileName(doc.path) }), started: bt("batch.portfolio.extractStarted"),
    });
    if (id !== null) p.onClose();
  };

  return (
    <TaskPanel
      title={bt("batch.portfolio.title")} onClose={p.onClose} estimate={bt("batch.portfolio.hint")}
      primary={{ label: bt("batch.portfolio.create"), icon: "folders", disabled: files().length === 0, onClick: () => void create() }}
      secondary={[{ label: bt("batch.portfolio.add"), icon: "file-plus", onClick: () => void add() }]}
      notice={<Show when={error()}><Notice tone="danger" inline>{error()}</Notice></Show>}
    >
      <p class="pl-field__label">{bt("batch.portfolio.files")}</p>
      <ul class="pl-index-hits">
        <For each={files()}>{(f) => (
          <li>
            <span title={f}>{fileName(f)}</span>
            <IconButton icon="x" label={bt("batch.portfolio.remove")} size="sm" onClick={() => setFiles((all) => all.filter((x) => x !== f))} />
          </li>
        )}</For>
      </ul>
      <Show when={listing()}>
        {(l) => (
          <>
            <p class="pl-field__label">{bt("batch.portfolio.inside")}</p>
            <Show when={l().is_portfolio}><Notice tone="info" inline>{bt("batch.portfolio.isPortfolio")}</Notice></Show>
            <Show when={l().entries.length} fallback={<p>{bt("batch.portfolio.none")}</p>}>
              <ul class="pl-index-hits">
                <For each={l().entries}>{(e) => <li>{e.filename} <small>({sizeLabel(e.size)})</small></li>}</For>
              </ul>
              <Button onClick={() => void extract()}>{bt("batch.portfolio.extract")}</Button>
            </Show>
          </>
        )}
      </Show>
    </TaskPanel>
  );
}
