import { createSignal, For } from "solid-js";
import { Field, Select, TaskPanel, TextInput } from "../../ui";
import { ColorField, FONTS, PagesField, useStampRun } from "./common";
import { st } from "./text";
import { headerFooterOp } from "./ops";
import { intIn, validPages } from "./util";

export default function HeaderFooterPanel(p: { onClose: () => void }) {
  const [hl, setHl] = createSignal(""); const [hc, setHc] = createSignal(""); const [hr, setHr] = createSignal("");
  const [fl, setFl] = createSignal(""); const [fc, setFc] = createSignal("{page} / {pages}"); const [fr, setFr] = createSignal("");
  const [font, setFont] = createSignal("Helvetica");
  const [size, setSize] = createSignal("10");
  const [color, setColor] = createSignal("#000000");
  const [margin, setMargin] = createSignal("36");
  const [pages, setPages] = createSignal("");

  const anyText = () => [hl(), hc(), hr(), fl(), fc(), fr()].some((t) => t.trim() !== "");
  const valid = () => anyText() && validPages(pages()) && intIn(size(), 4, 100) !== null && intIn(margin(), 0, 300) !== null;

  const op = () => headerFooterOp({
    header: [hl(), hc(), hr()], footer: [fl(), fc(), fr()], style: { font: font(), size: size(), color: color() }, margin: margin(),
  });
  const { run } = useStampRun("stamp.hf.job", op, pages, () => p.onClose());

  const slots = (title: string, vals: [() => string, (v: string) => void][]) => (
    <fieldset class="pl-stamp-slots">
      <legend class="pl-field__label">{title}</legend>
      <div class="pl-stamp-row">
        <For each={["left", "center", "right"] as const}>{(k, i) => (
          <Field label={st(`stamp.hf.${k}`)}><TextInput value={vals[i()]![0]()} onInput={(e) => vals[i()]![1](e.currentTarget.value)} /></Field>
        )}</For>
      </div>
    </fieldset>
  );

  return (
    <TaskPanel
      title={st("stamp.hf.title")} onClose={p.onClose} estimate={anyText() ? st("stamp.hint") : st("stamp.hf.empty")}
      primary={{ label: st("stamp.hf.run"), icon: "panel-top", disabled: !valid(), onClick: () => void run() }}
    >
      {slots(st("stamp.hf.header"), [[hl, setHl], [hc, setHc], [hr, setHr]])}
      {slots(st("stamp.hf.footer"), [[fl, setFl], [fc, setFc], [fr, setFr]])}
      <p class="pl-field__hint">{st("stamp.hf.vars")}</p>
      <Field label={st("stamp.font")}><Select value={font()} onChange={(e) => setFont(e.currentTarget.value)} options={FONTS()} /></Field>
      <Field label={st("stamp.size")} error={intIn(size(), 4, 100) === null ? st("stamp.invalid") : undefined}><TextInput type="number" value={size()} onInput={(e) => setSize(e.currentTarget.value)} /></Field>
      <ColorField label={st("stamp.color")} value={color()} onInput={setColor} />
      <Field label={st("stamp.margin")} error={intIn(margin(), 0, 300) === null ? st("stamp.invalid") : undefined}><TextInput type="number" value={margin()} onInput={(e) => setMargin(e.currentTarget.value)} /></Field>
      <PagesField value={pages()} onInput={setPages} />
    </TaskPanel>
  );
}
