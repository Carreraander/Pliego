import { createSignal } from "solid-js";
import { activeDoc } from "../../state/documents";
import { Checkbox, Field, Select, TaskPanel, TextInput } from "../../ui";
import { fileName, runToolJob } from "../run";
import { NUMBER_TEMPLATE, numberArgs, type NumberFormat } from "./request";
import { ot } from "./text";

/** Numerar páginas: imprime el número en la hoja (con `pliego-stamp`, como Encabezado y pie); guarda una copia. */
export default function NumberPanel(p: { onClose: () => void }) {
  const [position, setPosition] = createSignal("footer_center");
  const [format, setFormat] = createSignal<NumberFormat>("number");
  const [from, setFrom] = createSignal("1");
  const [size, setSize] = createSignal("10");
  const [labels, setLabels] = createSignal(false);
  const total = () => activeDoc()?.sizes.length ?? 0;
  const args = () => numberArgs({
    path: activeDoc()?.path ?? "", total: total(), position: position(), format: NUMBER_TEMPLATE[format()](ot("organize.number.word"), ot("organize.number.of")),
    from: from(), size: size(), alsoLabels: labels(), suffix: ot("organize.number.suffix"),
  });

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
      advanced={<>
        <Field label={ot("organize.number.size")}><TextInput type="number" min="4" max="72" value={size()} onInput={(e) => setSize(e.currentTarget.value)} /></Field>
        <Checkbox label={ot("organize.number.labels")} checked={labels()} onChange={(e) => setLabels(e.currentTarget.checked)} />
      </>}
    >
      <Field label={ot("organize.number.position")}>
        <Select value={position()} onChange={(e) => setPosition(e.currentTarget.value)} options={[
          { value: "footer_center", label: ot("organize.number.footerCenter") }, { value: "footer_right", label: ot("organize.number.footerRight") }, { value: "footer_left", label: ot("organize.number.footerLeft") },
          { value: "header_center", label: ot("organize.number.headerCenter") }, { value: "header_right", label: ot("organize.number.headerRight") }, { value: "header_left", label: ot("organize.number.headerLeft") },
        ]} />
      </Field>
      <Field label={ot("organize.number.format")}>
        <Select value={format()} onChange={(e) => setFormat(e.currentTarget.value as NumberFormat)} options={[
          { value: "number", label: "1" }, { value: "of", label: `1 ${ot("organize.number.of")} N` }, { value: "word", label: `${ot("organize.number.word")} 1` },
        ]} />
      </Field>
      <Field label={ot("organize.number.from")}><TextInput type="number" min="1" max={total()} value={from()} onInput={(e) => setFrom(e.currentTarget.value)} /></Field>
    </TaskPanel>
  );
}
