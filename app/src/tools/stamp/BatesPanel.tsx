import { createSignal } from "solid-js";
import { Field, Select, TaskPanel, TextInput } from "../../ui";
import { ColorField, ErrorNotice, FONTS, PagesField, useStampRun } from "./common";
import { st } from "./text";
import { batesOp } from "./ops";
import { batesSample, intIn, validPages } from "./util";

const CORNERS = ["top-left", "top-right", "bottom-left", "bottom-right"] as const;

export default function BatesPanel(p: { onClose: () => void }) {
  const [prefix, setPrefix] = createSignal("");
  const [suffix, setSuffix] = createSignal("");
  const [digits, setDigits] = createSignal("6");
  const [start, setStart] = createSignal("1");
  const [corner, setCorner] = createSignal<(typeof CORNERS)[number]>("bottom-right");
  const [font, setFont] = createSignal("Helvetica");
  const [size, setSize] = createSignal("10");
  const [color, setColor] = createSignal("#000000");
  const [pages, setPages] = createSignal("");

  const valid = () => validPages(pages()) && intIn(digits(), 1, 12) !== null && intIn(start(), 0, 999_999_999) !== null && intIn(size(), 4, 100) !== null;
  const op = () => batesOp({
    prefix: prefix(), suffix: suffix(), digits: digits(), start: start(), corner: corner(), style: { font: font(), size: size(), color: color() },
  });
  const { error, run } = useStampRun("stamp.bates.job", op, pages, () => p.onClose());
  const bad = (v: string, a: number, b: number) => (intIn(v, a, b) === null ? st("stamp.invalid") : undefined);

  return (
    <TaskPanel
      title={st("stamp.bates.title")} onClose={p.onClose}
      estimate={valid() ? st("stamp.bates.preview", { sample: batesSample(prefix(), suffix(), Number(digits()), Number(start())) }) : st("stamp.invalid")}
      primary={{ label: st("stamp.bates.run"), icon: "bates", disabled: !valid(), onClick: () => void run() }}
      notice={<ErrorNotice error={error()} />}
    >
      <Field label={st("stamp.bates.prefix")}><TextInput value={prefix()} onInput={(e) => setPrefix(e.currentTarget.value)} /></Field>
      <Field label={st("stamp.bates.suffix")}><TextInput value={suffix()} onInput={(e) => setSuffix(e.currentTarget.value)} /></Field>
      <Field label={st("stamp.bates.digits")} error={bad(digits(), 1, 12)}><TextInput type="number" value={digits()} onInput={(e) => setDigits(e.currentTarget.value)} /></Field>
      <Field label={st("stamp.bates.start")} error={bad(start(), 0, 999_999_999)}><TextInput type="number" value={start()} onInput={(e) => setStart(e.currentTarget.value)} /></Field>
      <Field label={st("stamp.bates.corner")}>
        <Select value={corner()} onChange={(e) => setCorner(e.currentTarget.value as (typeof CORNERS)[number])} options={CORNERS.map((c) => ({ value: c, label: st(`stamp.corner.${c}`) }))} />
      </Field>
      <Field label={st("stamp.font")}><Select value={font()} onChange={(e) => setFont(e.currentTarget.value)} options={FONTS()} /></Field>
      <Field label={st("stamp.size")} error={bad(size(), 4, 100)}><TextInput type="number" value={size()} onInput={(e) => setSize(e.currentTarget.value)} /></Field>
      <ColorField label={st("stamp.color")} value={color()} onInput={setColor} />
      <PagesField value={pages()} onInput={setPages} />
    </TaskPanel>
  );
}
