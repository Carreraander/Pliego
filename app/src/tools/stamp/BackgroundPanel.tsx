import { createSignal, Show } from "solid-js";
import { Field, Select, Slider, TaskPanel } from "../../ui";
import { ColorField, ImageField, PagesField, useStampRun } from "./common";
import { st } from "./text";
import { backgroundOp } from "./ops";
import { validPages } from "./util";

export default function BackgroundPanel(p: { onClose: () => void }) {
  const [kind, setKind] = createSignal<"color" | "image">("color");
  const [color, setColor] = createSignal("#fff4cc");
  const [image, setImage] = createSignal<string | null>(null);
  const [opacity, setOpacity] = createSignal(100);
  const [pages, setPages] = createSignal("");

  const valid = () => validPages(pages()) && (kind() === "color" || image() !== null);
  const op = () => backgroundOp({ kind: kind(), color: color(), image: image(), opacityPct: opacity() });
  const { run } = useStampRun("stamp.bg.job", op, pages, () => p.onClose());

  return (
    <TaskPanel
      title={st("stamp.bg.title")} onClose={p.onClose} estimate={st("stamp.hint")}
      primary={{ label: st("stamp.bg.run"), icon: "image", disabled: !valid(), onClick: () => void run() }}
    >
      <Field label={st("stamp.bg.kind")}>
        <Select value={kind()} onChange={(e) => setKind(e.currentTarget.value as "color" | "image")}
          options={[{ value: "color", label: st("stamp.bg.color") }, { value: "image", label: st("stamp.image") }]} />
      </Field>
      <Show when={kind() === "color"} fallback={<ImageField path={image()} onPick={setImage} />}>
        <ColorField label={st("stamp.color")} value={color()} onInput={setColor} />
      </Show>
      <Field label={st("stamp.opacity")}><Slider min="5" max="100" value={opacity()} valueLabel={`${opacity()} %`} onInput={(e) => setOpacity(Number(e.currentTarget.value))} /></Field>
      <PagesField value={pages()} onInput={setPages} />
    </TaskPanel>
  );
}
