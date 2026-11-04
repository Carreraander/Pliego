import { createSignal, Show } from "solid-js";
import { locale } from "../../i18n";
import { activeDoc } from "../../state/documents";
import { Notice, Radio, RadioGroup, TaskPanel } from "../../ui";
import { fileName, formatBytes, runToolJob } from "../run";
import { compressArgs, QUALITY_PRESET, type CompressQuality } from "./request";
import { ot } from "./strings";
import { createUsage } from "./usage";

/** Comprimir: tres calidades sobre los preajustes de `pliego-optimize`; guarda una copia y el original no cambia. */
export default function CompressPanel(p: { onClose: () => void }) {
  const [quality, setQuality] = createSignal<CompressQuality>("balanced");
  const [usage] = createUsage();
  const doc = activeDoc;
  const apply = async () => {
    const d = doc();
    if (!d) return;
    const id = await runToolJob({
      command: "compress_pdf",
      args: compressArgs(d.path, QUALITY_PRESET[quality()], ot("optimize.suffix")),
      title: ot("optimize.job", { name: fileName(d.path) }),
      started: ot("optimize.started"),
    });
    if (id !== null) p.onClose();
  };
  const stem = () => fileName(doc()?.path ?? "").replace(/\.pdf$/i, "");
  return (
    <TaskPanel
      title={ot("optimize.title")}
      onClose={p.onClose}
      estimate={usage() ? ot("optimize.current", { size: formatBytes(usage()!.total_bytes, locale() === "es") }) : undefined}
      primary={{ label: ot("optimize.save"), onClick: apply, disabled: !doc() }}
    >
      <Show when={!doc()}><Notice tone="warn" inline>{ot("optimize.noDoc")}</Notice></Show>
      <RadioGroup legend={ot("optimize.quality")} value={quality()} onChange={(v) => setQuality(v as CompressQuality)}>
        <Radio value="max" label={ot("optimize.max")} hint={ot("optimize.max.hint")} />
        <Radio value="balanced" label={ot("optimize.balanced")} hint={ot("optimize.balanced.hint")} />
        <Radio value="min" label={ot("optimize.min")} hint={ot("optimize.min.hint")} />
      </RadioGroup>
      <p class="pl-muted">{ot("optimize.note", { name: `${stem()} (${ot("optimize.suffix")}).pdf` })}</p>
    </TaskPanel>
  );
}
