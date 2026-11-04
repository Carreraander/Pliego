import { createSignal, For, Show } from "solid-js";
import { locale } from "../../i18n";
import { activeDoc } from "../../state/documents";
import { Field, Notice, Select, TaskPanel, TextInput } from "../../ui";
import { errorText, fileName, formatBytes, runToolJob } from "../run";
import { compressArgs, parseOptional, type OptimizePreset } from "./request";
import { ot } from "./strings";
import { createUsage } from "./usage";

/** Optimizar avanzado: preajuste, resolución y calidad JPEG, y cómo se reparte el tamaño del archivo. */
export default function AdvancedPanel(p: { onClose: () => void }) {
  const [preset, setPreset] = createSignal<OptimizePreset>("ebook");
  const [dpi, setDpi] = createSignal("");
  const [quality, setQuality] = createSignal("");
  const [usage] = createUsage();
  const doc = activeDoc;
  const size = (n: number) => formatBytes(n, locale() === "es");
  const rows = () => {
    const u = usage();
    return u ? [
      [ot("optimize.adv.images"), u.images_bytes], [ot("optimize.adv.fonts"), u.fonts_bytes],
      [ot("optimize.adv.content"), u.content_bytes], [ot("optimize.adv.other"), u.other_streams_bytes],
      [ot("optimize.adv.structure"), u.structure_bytes],
    ] as [string, number][] : [];
  };
  const lossless = () => preset() === "sin-perdida";
  const apply = async () => {
    const d = doc();
    if (!d) return;
    const id = await runToolJob({
      command: "compress_pdf",
      args: compressArgs(d.path, preset(), ot("optimize.suffix"), lossless() ? null : parseOptional(dpi(), 36, 1200), lossless() ? null : parseOptional(quality(), 1, 100)),
      title: ot("optimize.job", { name: fileName(d.path) }),
      started: ot("optimize.started"),
    });
    if (id !== null) p.onClose();
  };
  return (
    <TaskPanel title={ot("optimize.adv.title")} onClose={p.onClose} primary={{ label: ot("optimize.adv.save"), onClick: apply, disabled: !doc() }}
      estimate={usage() ? ot("optimize.current", { size: size(usage()!.total_bytes) }) : undefined}>
      <Show when={!doc()}><Notice tone="warn" inline>{ot("optimize.noDoc")}</Notice></Show>
      <Field label={ot("optimize.adv.preset")}>
        <Select value={preset()} onChange={(e) => setPreset(e.currentTarget.value as OptimizePreset)} options={[
          { value: "pantalla", label: ot("optimize.min") }, { value: "ebook", label: ot("optimize.balanced") },
          { value: "impresion", label: ot("optimize.max") }, { value: "sin-perdida", label: ot("optimize.adv.lossless") },
        ]} />
      </Field>
      <Field label={ot("optimize.adv.dpi")} hint={ot("optimize.adv.dpi.hint")}>
        <TextInput inputmode="decimal" value={dpi()} disabled={lossless()} onInput={(e) => setDpi(e.currentTarget.value)} />
      </Field>
      <Field label={ot("optimize.adv.quality")} hint={ot("optimize.adv.quality.hint")}>
        <TextInput inputmode="numeric" value={quality()} disabled={lossless()} onInput={(e) => setQuality(e.currentTarget.value)} />
      </Field>
      <h3 class="pl-field__label">{ot("optimize.adv.usage")}</h3>
      <Show when={!usage.error} fallback={<Notice tone="warn" inline>{ot("optimize.adv.failed", { error: errorText(usage.error) })}</Notice>}>
        <Show when={usage()} fallback={<p class="pl-muted">{ot("optimize.adv.analyzing")}</p>}>
          <dl class="pl-num">
            <For each={rows()}>{([label, bytes]) => <div style={{ display: "flex", "justify-content": "space-between" }}><dt>{label}</dt><dd>{size(bytes)}</dd></div>}</For>
          </dl>
        </Show>
      </Show>
    </TaskPanel>
  );
}
