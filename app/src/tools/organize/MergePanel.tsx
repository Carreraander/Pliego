import "./organize.css";
import { createSignal, For, Show } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { api, isApiError } from "../../api";
import { trackJob } from "../../jobs";
import { activeDoc } from "../../state/documents";
import { Checkbox, IconButton, Notice, TaskPanel, toast } from "../../ui";
import { ot } from "./text";

const base = (p: string) => p.split(/[\\/]/).pop() ?? p;

export default function MergePanel(p: { onClose: () => void }) {
  const [files, setFiles] = createSignal<string[]>(activeDoc() ? [activeDoc()!.path] : []);
  const [perFile, setPerFile] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  const add = async () => {
    const picked = await api.pickPdfFiles();
    if (picked.length) setFiles((f) => [...f, ...picked.filter((x) => !f.includes(x))]);
  };
  const move = (i: number, d: -1 | 1) => setFiles((f) => {
    const j = i + d;
    if (j < 0 || j >= f.length) return f;
    const next = [...f];
    [next[i], next[j]] = [next[j]!, next[i]!];
    return next;
  });
  const run = async () => {
    const paths = files();
    try {
      const id = await invoke<number>("merge_pdfs", { paths, options: { keep_bookmarks: true, bookmark_per_file: perFile() } });
      trackJob(id, { title: ot("organize.merge.job", { n: paths.length }), retry: () => void run() });
      toast({ tone: "info", text: ot("organize.merge.started", { n: paths.length }) });
      p.onClose();
    } catch (e) {
      setError(isApiError(e) ? e.message : String(e));
    }
  };

  return (
    <TaskPanel
      title={ot("organize.merge.title")} onClose={p.onClose}
      primary={{ label: ot("organize.merge.run"), icon: "combine", disabled: files().length < 2, onClick: () => void run() }}
      secondary={[{ label: ot("organize.add"), icon: "file-plus", onClick: () => void add() }]}
      estimate={files().length < 2 ? ot("organize.merge.min") : ot("organize.merge.hint")}
      notice={<Show when={error()}><Notice tone="danger" inline>{error()}</Notice></Show>}
    >
      <p class="pl-field__label">{ot("organize.merge.files")}</p>
      <ol class="pl-organize-files">
        <For each={files()}>{(f, i) => (
          <li>
            <span title={f}>{base(f)}</span>
            <IconButton icon="chevron-up" label={ot("organize.up")} size="sm" disabled={i() === 0} onClick={() => move(i(), -1)} />
            <IconButton icon="chevron-down" label={ot("organize.down")} size="sm" disabled={i() === files().length - 1} onClick={() => move(i(), 1)} />
            <IconButton icon="x" label={ot("organize.remove")} size="sm" onClick={() => setFiles((all) => all.filter((_, k) => k !== i()))} />
          </li>
        )}</For>
      </ol>
      <Checkbox label={ot("organize.merge.bookmarks")} checked={perFile()} onChange={(e) => setPerFile(e.currentTarget.checked)} />
    </TaskPanel>
  );
}
