import { createSignal, For, Show } from "solid-js";
import { currentPageIndex } from "../../state/documents";
import { Field, Radio, RadioGroup, Select, TaskPanel, TextInput } from "../../ui";
import { ColorField, ImageField, useStampRun } from "./common";
import { PLACES, PRESETS, stampPages, stampRequest, type Place, type Preset } from "./stampsOps";
import { st } from "./text";

/** Sellos: predefinido, texto o imagen, en el centro o una esquina de la página actual o de todas. Guarda una copia. */
export default function StampsPanel(p: { onClose: () => void }) {
  const [kind, setKind] = createSignal<"preset" | "text" | "image">("preset");
  const [preset, setPreset] = createSignal<Preset>("aprobado");
  const [text, setText] = createSignal("");
  const [color, setColor] = createSignal("#1a3fa0");
  const [image, setImage] = createSignal<string | null>(null);
  const [place, setPlace] = createSignal<Place>("top_right");
  const [scope, setScope] = createSignal<"current" | "all">("current");

  const valid = () => kind() === "preset" || (kind() === "text" ? text().trim() !== "" : image() !== null);
  const request = () => stampRequest({ kind: kind(), preset: preset(), text: text(), color: color(), image: image(), place: place() });
  const { run } = useStampRun("stamp.stamps.job", request, () => stampPages(scope(), currentPageIndex()), () => p.onClose(), "stamp_annot_pdf", "request");

  return (
    <TaskPanel
      title={st("stamp.stamps.title")} onClose={p.onClose} estimate={st("stamp.hint")}
      primary={{ label: st("stamp.stamps.run"), icon: "stamp", disabled: !valid(), onClick: () => void run() }}
    >
      <Field label={st("stamp.wm.kind")}>
        <Select value={kind()} onChange={(e) => setKind(e.currentTarget.value as "preset" | "text" | "image")}
          options={[{ value: "preset", label: st("stamp.stamps.preset") }, { value: "text", label: st("stamp.wm.text") }, { value: "image", label: st("stamp.image") }]} />
      </Field>
      <Show when={kind() === "preset"}>
        <Field label={st("stamp.stamps.preset")}>
          <Select value={preset()} onChange={(e) => setPreset(e.currentTarget.value as Preset)} options={PRESETS.map((v) => ({ value: v, label: st(`stamp.preset.${v}`) }))} />
        </Field>
      </Show>
      <Show when={kind() === "text"}>
        <Field label={st("stamp.wm.textLabel")}><TextInput value={text()} onInput={(e) => setText(e.currentTarget.value)} /></Field>
        <ColorField label={st("stamp.color")} value={color()} onInput={setColor} />
      </Show>
      <Show when={kind() === "image"}><ImageField path={image()} onPick={setImage} /></Show>
      <Field label={st("stamp.stamps.place")}>
        <Select value={place()} onChange={(e) => setPlace(e.currentTarget.value as Place)} options={PLACES.map((v) => ({ value: v, label: st(`stamp.place.${v}`) }))} />
      </Field>
      <RadioGroup legend={st("stamp.pages")} value={scope()} onChange={(v) => setScope(v as "current" | "all")}>
        <For each={["current", "all"] as const}>{(v) => <Radio value={v} label={st(`stamp.stamps.scope.${v}`)} />}</For>
      </RadioGroup>
    </TaskPanel>
  );
}
