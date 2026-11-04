import { createSignal, Show } from "solid-js";
import { activeDoc } from "../../state/documents";
import { Notice, Radio, RadioGroup, TaskPanel } from "../../ui";
import { fileName, runToolJob } from "../run";
import { pdfaArgs, type PdfaLevel } from "./request";
import { ct } from "./strings";

/** Guardar como PDF/A (1b, 2b, 2u, 3b): copia de larga conservación junto al original. */
export default function PdfaPanel(p: { onClose: () => void }) {
  const [level, setLevel] = createSignal<PdfaLevel>("2b");
  const doc = activeDoc;
  const run = async () => {
    const d = doc();
    if (!d) return;
    const id = await runToolJob({
      command: "pdfa_convert",
      args: pdfaArgs(d.path, level(), ct("convert.pdfa.label")),
      title: ct("convert.pdfa.job", { name: fileName(d.path) }),
      started: ct("convert.pdfa.started"),
    });
    if (id !== null) p.onClose();
  };
  const levels: PdfaLevel[] = ["2b", "2u", "1b", "3b"];
  return (
    <TaskPanel title={ct("convert.pdfa.title")} onClose={p.onClose} primary={{ label: ct("convert.pdfa.run"), onClick: run, disabled: !doc() }}
      notice={<Show when={!doc()}><Notice tone="warn" inline>{ct("convert.noDoc")}</Notice></Show>}>
      <RadioGroup legend={ct("convert.pdfa.level")} value={level()} onChange={(v) => setLevel(v as PdfaLevel)}>
        {levels.map((l) => <Radio value={l} label={ct(`convert.pdfa.${l}`)} hint={ct(`convert.pdfa.${l}.hint`)} />)}
      </RadioGroup>
      <p class="pl-muted">{ct("convert.pdfa.note")}</p>
    </TaskPanel>
  );
}
