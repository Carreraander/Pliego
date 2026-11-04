import { createSignal, Show } from "solid-js";
import { Checkbox, Field, Select, Slider, TaskPanel, TextInput } from "../../ui";
import { ColorField, ErrorNotice, FONTS, ImageField, PagesField, useStampRun } from "./common";
import { st } from "./text";
import { watermarkOp } from "./ops";
import { intIn, numIn, validPages } from "./util";

export default function WatermarkPanel(p: { onClose: () => void }) {
  const [kind, setKind] = createSignal<"text" | "image">("text");
  const [text, setText] = createSignal(st("stamp.wm.textDefault"));
  const [font, setFont] = createSignal("Helvetica");
  const [size, setSize] = createSignal("72");
  const [color, setColor] = createSignal("#808080");
  const [image, setImage] = createSignal<string | null>(null);
  const [scale, setScale] = createSignal(50);
  const [opacity, setOpacity] = createSignal(30);
  const [rotation, setRotation] = createSignal("45");
  const [behind, setBehind] = createSignal(false);
  const [pages, setPages] = createSignal("");

  const valid = () =>
    validPages(pages()) &&
    (kind() === "text" ? text().trim() !== "" && intIn(size(), 4, 400) !== null && numIn(rotation(), -360, 360) !== null : image() !== null);

  const op = () => watermarkOp({
    kind: kind(), text: text(), style: { font: font(), size: size(), color: color() }, image: image(), scalePct: scale(),
    opacityPct: opacity(), rotation: rotation(), behind: behind(),
  });
  const { error, run } = useStampRun("stamp.wm.job", op, pages, () => p.onClose());

  return (
    <TaskPanel
      title={st("stamp.wm.title")} onClose={p.onClose} estimate={st("stamp.hint")}
      primary={{ label: st("stamp.wm.run"), icon: "droplet", disabled: !valid(), onClick: () => void run() }}
      notice={<ErrorNotice error={error()} />}
    >
      <Field label={st("stamp.wm.kind")}>
        <Select value={kind()} onChange={(e) => setKind(e.currentTarget.value as "text" | "image")}
          options={[{ value: "text", label: st("stamp.wm.text") }, { value: "image", label: st("stamp.image") }]} />
      </Field>
      <Show when={kind() === "text"} fallback={
        <>
          <ImageField path={image()} onPick={setImage} />
          <Field label={st("stamp.wm.scale")}><Slider min="10" max="100" value={scale()} valueLabel={`${scale()} %`} onInput={(e) => setScale(Number(e.currentTarget.value))} /></Field>
        </>
      }>
        <Field label={st("stamp.wm.textLabel")}><TextInput value={text()} onInput={(e) => setText(e.currentTarget.value)} /></Field>
        <Field label={st("stamp.font")}><Select value={font()} onChange={(e) => setFont(e.currentTarget.value)} options={FONTS()} /></Field>
        <Field label={st("stamp.size")} error={intIn(size(), 4, 400) === null ? st("stamp.invalid") : undefined}><TextInput type="number" value={size()} onInput={(e) => setSize(e.currentTarget.value)} /></Field>
        <ColorField label={st("stamp.color")} value={color()} onInput={setColor} />
        <Field label={st("stamp.wm.rotation")} error={numIn(rotation(), -360, 360) === null ? st("stamp.invalid") : undefined}><TextInput type="number" value={rotation()} onInput={(e) => setRotation(e.currentTarget.value)} /></Field>
      </Show>
      <Field label={st("stamp.opacity")}><Slider min="5" max="100" value={opacity()} valueLabel={`${opacity()} %`} onInput={(e) => setOpacity(Number(e.currentTarget.value))} /></Field>
      <Checkbox label={st("stamp.wm.behind")} checked={behind()} onChange={(e) => setBehind(e.currentTarget.checked)} />
      <PagesField value={pages()} onInput={setPages} />
    </TaskPanel>
  );
}
