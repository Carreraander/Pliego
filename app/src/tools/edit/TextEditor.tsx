import { createSignal } from "solid-js";
import { Button, Checkbox, Field, Select, TextInput } from "../../ui";
import { parseSize, styleFor, type Align, type Family, type TextBlock, type TextStyle } from "./geometry";
import { et } from "./text";

/** Formulario emergente junto al texto: contenido y estilo. `block` es el párrafo que se edita (`null` = texto nuevo). */
export function TextEditor(p: { block: TextBlock | null; style: { left: string; top: string }; onApply: (text: string, style: TextStyle) => void; onCancel: () => void }) {
  const [text, setText] = createSignal(p.block?.text ?? "");
  const [family, setFamily] = createSignal<Family>(p.block?.family ?? "sans");
  const [size, setSize] = createSignal(String(Math.round((p.block?.font_size ?? 12) * 10) / 10));
  const [bold, setBold] = createSignal(p.block?.bold ?? false);
  const [italic, setItalic] = createSignal(p.block?.italic ?? false);
  const [align, setAlign] = createSignal<Align>("left");
  const n = () => parseSize(size());
  const submit = () => {
    const sz = n();
    if (sz === null || !text().trim()) return;
    p.onApply(text(), styleFor(p.block, { family: family(), size: sz, bold: bold(), italic: italic(), align: align() }));
  };
  return (
    <form class="pl-edit-form" style={p.style} onPointerDown={(e) => e.stopPropagation()}
      onSubmit={(e) => { e.preventDefault(); submit(); }} onKeyDown={(e) => { if (e.key === "Escape") p.onCancel(); }}>
      <textarea autofocus class="pl-input" rows="5" aria-label={et("edit.editor.label")} value={text()} onInput={(e) => setText(e.currentTarget.value)} />
      <div class="pl-edit-form__row">
        <Field label={et("edit.editor.family")}>
          <Select value={family()} onChange={(e) => setFamily(e.currentTarget.value as Family)}
            options={(["sans", "serif", "mono"] as Family[]).map((f) => ({ value: f, label: et("edit.family." + f) }))} />
        </Field>
        <Field label={et("edit.editor.size")} error={n() === null ? et("edit.editor.badSize") : undefined}>
          <TextInput inputmode="decimal" value={size()} onInput={(e) => setSize(e.currentTarget.value)} />
        </Field>
        <Field label={et("edit.editor.align")}>
          <Select value={align()} onChange={(e) => setAlign(e.currentTarget.value as Align)}
            options={(["left", "center", "right"] as Align[]).map((a) => ({ value: a, label: et("edit.align." + a) }))} />
        </Field>
      </div>
      <div class="pl-edit-form__row">
        <Checkbox label={et("edit.editor.bold")} checked={bold()} onChange={(e) => setBold(e.currentTarget.checked)} />
        <Checkbox label={et("edit.editor.italic")} checked={italic()} onChange={(e) => setItalic(e.currentTarget.checked)} />
      </div>
      <div class="pl-edit-form__actions">
        <Button variant="ghost" type="button" onClick={p.onCancel}>{et("edit.editor.cancel")}</Button>
        <Button variant="primary" type="submit" disabled={n() === null || !text().trim()}>{et("edit.editor.apply")}</Button>
      </div>
    </form>
  );
}
