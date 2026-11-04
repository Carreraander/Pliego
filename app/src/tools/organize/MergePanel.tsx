import "./organize.css";
import { createSignal, For } from "solid-js";
import { api } from "../../api";
import { activeDoc } from "../../state/documents";
import { Checkbox, IconButton, TaskPanel } from "../../ui";
import { fileName, runToolJob } from "../run";
import { ot } from "./text";

const base = fileName;

export default function MergePanel(p: { onClose: () => void }) {
  const [files, setFiles] = createSignal<string[]>(activeDoc() ? [activeDoc()!.path] : []);
  const [perFile, setPerFile] = createSignal(false);

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
    const id = await runToolJob({
      command: "merge_pdfs",
      args: { paths, options: { keep_bookmarks: true, bookmark_per_file: perFile() }, suffix: ot("organize.merge.suffix") },
      title: ot("organize.merge.job", { n: paths.length }),
      started: ot("organize.merge.started", { n: paths.length }),
    });
    if (id !== null) p.onClose();
  };

  return (
    <TaskPanel
      title={ot("organize.merge.title")} onClose={p.onClose}
      primary={{ label: ot("organize.merge.run"), icon: "combine", disabled: files().length < 2, onClick: () => void run() }}
      secondary={[{ label: ot("organize.add"), icon: "file-plus", onClick: () => void add() }]}
      estimate={files().length < 2 ? ot("organize.merge.min") : ot("organize.merge.hint")}
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
