import { createSignal } from "solid-js";
import { activeDoc } from "../../state/documents";
import { Field, Select, TaskPanel, TextInput } from "../../ui";
import { fileName, runToolJob } from "../run";
import { numberArgs } from "./request";
import { ot } from "./text";

/** Numeración de páginas del PDF (etiquetas): 1, 2, 3 · i, ii, iii · A, B, C…; guarda una copia. */
export default function NumberPanel(p: { onClose: () => void }) {
  const [style, setStyle] = createSignal("decimal");
  const [prefix, setPrefix] = createSignal("");
  const [first, setFirst] = createSignal("1");
  const [from, setFrom] = createSignal("1");
  const total = () => activeDoc()?.sizes.length ?? 0;
  const args = () => numberArgs({ path: activeDoc()?.path ?? "", total: total(), style: style(), prefix: prefix(), first: first(), from: from(), suffix: ot("organize.number.suffix") });

  const run = async () => {
    const a = args();
    const d = activeDoc();
    if (!a || !d) return;
    const id = await runToolJob({ command: "number_pages", args: a, title: ot("organize.number.job", { name: fileName(d.path) }), started: ot("organize.number.started") });
    if (id !== null) p.onClose();
  };

  return (
    <TaskPanel
      title={ot("organize.number.title")} onClose={p.onClose}
      primary={{ label: ot("organize.number.run"), icon: "hash", disabled: args() === null, onClick: () => void run() }}
      estimate={ot("organize.number.hint")}
    >
      <Field label={ot("organize.number.style")}>
        <Select value={style()} onChange={(e) => setStyle(e.currentTarget.value)} options={[
          { value: "decimal", label: "1, 2, 3" }, { value: "lower_roman", label: "i, ii, iii" }, { value: "upper_roman", label: "I, II, III" },
          { value: "lower_alpha", label: "a, b, c" }, { value: "upper_alpha", label: "A, B, C" },
        ]} />
      </Field>
      <Field label={ot("organize.number.prefix")} hint={ot("organize.number.prefixHint")}><TextInput value={prefix()} onInput={(e) => setPrefix(e.currentTarget.value)} /></Field>
      <Field label={ot("organize.number.from")}><TextInput type="number" min="1" max={total()} value={from()} onInput={(e) => setFrom(e.currentTarget.value)} /></Field>
      <Field label={ot("organize.number.first")}><TextInput type="number" min="1" value={first()} onInput={(e) => setFirst(e.currentTarget.value)} /></Field>
    </TaskPanel>
  );
}
