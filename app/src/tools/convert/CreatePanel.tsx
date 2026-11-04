import { createSignal, For, Show } from "solid-js";
import { api } from "../../api";
import { Checkbox, Field, IconButton, Notice, Select, TaskPanel, TextInput } from "../../ui";
import { fileName, runToolJob } from "../run";
import { createArgs, createProblem, optionalNumber, type CreateOrientation, type CreateSize } from "./request";
import { ct } from "./strings";

/** Crear PDF: imágenes (una página cada una), texto, Markdown, HTML o un documento de Office. */
export default function CreatePanel(p: { onClose: () => void }) {
  const [files, setFiles] = createSignal<string[]>([]);
  const [size, setSize] = createSignal<CreateSize>("fit");
  const [margin, setMargin] = createSignal("");
  const [orientation, setOrientation] = createSignal<CreateOrientation>("auto");
  const [upscale, setUpscale] = createSignal(false);
  const problem = () => createProblem(files());
  const add = async () => {
    const picked = await api.pickSourceFiles(ct("convert.create.add"));
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
      command: "create_pdf",
      args: createArgs(paths, size(), optionalNumber(margin(), 0, 100), orientation(), upscale(), ct("convert.create.label")),
      title: ct("convert.create.job", { name: fileName(paths[0] ?? "") }),
      started: ct("convert.create.started"),
    });
    if (id !== null) p.onClose();
  };
  const sizes: CreateSize[] = ["fit", "a4", "a3", "a5", "letter", "legal"];
  return (
    <TaskPanel title={ct("convert.create.title")} onClose={p.onClose}
      primary={{ label: ct("convert.create.run"), onClick: run, disabled: problem() !== null }}
      secondary={[{ label: ct("convert.create.add"), icon: "file-plus", onClick: () => void add() }]}
      notice={<Show when={problem() === "single"}><Notice tone="warn" inline>{ct("convert.create.single")}</Notice></Show>}>
      <p class="pl-field__label">{ct("convert.create.files")}</p>
      <Show when={files().length > 0} fallback={<p class="pl-muted">{ct("convert.create.empty")}</p>}>
        <ol>
          <For each={files()}>{(f, i) => (
            <li style={{ display: "flex", "align-items": "center", gap: "4px" }}>
              <span title={f} style={{ flex: 1, overflow: "hidden", "text-overflow": "ellipsis" }}>{fileName(f)}</span>
              <IconButton icon="chevron-up" label={ct("convert.create.up")} size="sm" disabled={i() === 0} onClick={() => move(i(), -1)} />
              <IconButton icon="chevron-down" label={ct("convert.create.down")} size="sm" disabled={i() === files().length - 1} onClick={() => move(i(), 1)} />
              <IconButton icon="x" label={ct("convert.create.remove")} size="sm" onClick={() => setFiles((all) => all.filter((_, k) => k !== i()))} />
            </li>
          )}</For>
        </ol>
      </Show>
      <Field label={ct("convert.create.size")}>
        <Select value={size()} onChange={(e) => setSize(e.currentTarget.value as CreateSize)}
          options={sizes.map((s) => ({ value: s, label: s === "fit" ? ct("convert.create.size.fit") : s === "letter" ? "Letter" : s === "legal" ? "Legal" : s.toUpperCase() }))} />
      </Field>
      <Field label={ct("convert.create.margin")}>
        <TextInput inputmode="decimal" value={margin()} onInput={(e) => setMargin(e.currentTarget.value)} />
      </Field>
      <Field label={ct("convert.create.orientation")}>
        <Select value={orientation()} onChange={(e) => setOrientation(e.currentTarget.value as CreateOrientation)}
          options={[{ value: "auto", label: ct("convert.create.auto") }, { value: "portrait", label: ct("convert.create.portrait") }, { value: "landscape", label: ct("convert.create.landscape") }]} />
      </Field>
      <Checkbox label={ct("convert.create.upscale")} checked={upscale()} onChange={(e) => setUpscale(e.currentTarget.checked)} />
      <p class="pl-muted">{ct("convert.create.note")}</p>
    </TaskPanel>
  );
}
