import "./organize.css";
import { createSignal, For } from "solid-js";
import { api } from "../../api";
import { activeDoc } from "../../state/documents";
import { passwordFor } from "../../state/passwords";
import { unlockFile } from "../../state/unlock";
import { Checkbox, IconButton, TaskPanel, toast } from "../../ui";
import { fileName, runToolJob } from "../run";
import { mergeOptions } from "./request";
import { ot } from "./text";

const base = fileName;

export default function MergePanel(p: { onClose: () => void }) {
  const [files, setFiles] = createSignal<string[]>(activeDoc() ? [activeDoc()!.path] : []);
  const [perFile, setPerFile] = createSignal(false);
  // Contraseña de cada archivo protegido (solo en memoria, mientras el panel está abierto). El documento abierto ya
  // trae la suya de cuando se abrió.
  const [passwords, setPasswords] = createSignal<Record<string, string>>(
    activeDoc() && passwordFor(activeDoc()!.path) ? { [activeDoc()!.path]: passwordFor(activeDoc()!.path)! } : {},
  );

  const add = async () => {
    const picked = (await api.pickPdfFiles()).filter((x) => !files().includes(x));
    // Cada archivo protegido pide su contraseña ahora; los que se cancelan no se añaden.
    const added: string[] = [];
    for (const path of picked) {
      const u = await unlockFile(path);
      if (!u.ok) { if (u.reason === "error") toast({ tone: "danger", text: u.message ?? "" }); continue; }
      if (u.password) setPasswords((all) => ({ ...all, [path]: u.password! }));
      added.push(path);
    }
    if (added.length) setFiles((f) => [...f, ...added]);
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
      args: { paths, options: mergeOptions(paths, perFile(), passwords()), suffix: ot("organize.merge.suffix") },
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
