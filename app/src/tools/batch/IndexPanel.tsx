import "./batch.css";
import { createSignal, For, Show } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { api } from "../../api";
import { openPath } from "../../state/documents";
import { Button, Field, Notice, TaskPanel, TextInput } from "../../ui";
import { errorText, fileName, runToolJob } from "../run";
import { snippetParts, type IndexHit } from "./hits";
import { bt } from "./strings";

/** Índice de búsqueda de una carpeta de PDF (trabajo en segundo plano) y búsqueda sobre él. */
export default function IndexPanel(p: { onClose: () => void }) {
  const [folder, setFolder] = createSignal<string | null>(null);
  const [query, setQuery] = createSignal("");
  const [hits, setHits] = createSignal<IndexHit[] | null>(null);
  const [error, setError] = createSignal<string | null>(null);

  const pick = async () => setFolder((await api.pickFolder(bt("batch.index.folder"))) ?? folder());
  const run = async () => {
    const f = folder();
    if (!f) return;
    const id = await runToolJob({
      command: "index_folder", args: { folder: f },
      title: bt("batch.index.job", { name: fileName(f) }), started: bt("batch.index.started"),
    });
    if (id !== null) p.onClose();
  };
  const search = async () => {
    setError(null);
    try { setHits(await invoke<IndexHit[]>("index_search", { query: query() })); }
    catch (e) { setHits(null); setError(bt("batch.index.failed", { error: errorText(e) })); }
  };
  const open = (path: string) => void openPath(path, window.innerWidth);

  return (
    <TaskPanel title={bt("batch.index.title")} onClose={p.onClose} estimate={bt("batch.index.hint")}
      primary={{ label: bt("batch.index.run"), icon: "database", disabled: !folder(), onClick: () => void run() }}
      notice={<Show when={error()}><Notice tone="danger" inline>{error()}</Notice></Show>}>
      <Field label={bt("batch.index.folder")} hint={folder() ?? bt("batch.index.none")}>
        <Button onClick={() => void pick()}>{bt("batch.index.pick")}</Button>
      </Field>
      <Field label={bt("batch.index.search")}>
        <TextInput value={query()} onInput={(e) => setQuery(e.currentTarget.value)} onKeyDown={(e) => { if (e.key === "Enter") void search(); }} />
      </Field>
      <Button onClick={() => void search()} disabled={!query().trim()}>{bt("batch.index.go")}</Button>
      <Show when={hits()}>
        {(list) => (
          <Show when={list().length} fallback={<Notice tone="info" inline>{bt("batch.index.empty")}</Notice>}>
            <ul class="pl-index-hits">
              <For each={list()}>
                {(h) => (
                  <li>
                    <button type="button" onClick={() => open(h.path)}>
                      <strong>{bt("batch.index.page", { name: fileName(h.path), n: h.page + 1 })}</strong>
                      <For each={snippetParts(h.snippet, h.highlights)}>{(s) => (s.mark ? <mark>{s.text}</mark> : <>{s.text}</>)}</For>
                    </button>
                  </li>
                )}
              </For>
            </ul>
          </Show>
        )}
      </Show>
    </TaskPanel>
  );
}
