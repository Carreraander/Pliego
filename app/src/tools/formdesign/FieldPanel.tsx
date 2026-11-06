import { createSignal, Show } from "solid-js";
import { locale } from "../../i18n";
import { Checkbox, Field, Notice, TaskPanel, TextInput } from "../../ui";
import { addField, deleteField, updateField } from "./edit";
import { freeName, nameError, newSpec, parseOptions, patchOf, type FieldInfo, type FieldKind } from "./geometry";
import { fieldList, setSelectedField, type Editing } from "./state";
import { ft } from "./text";

/** Propiedades de un campo en el inspector: crea uno recién dibujado o edita/borra uno existente. */
export default function FieldPanel(props: { editing: Editing; onClose: () => void }) {
  const p = props.editing;
  const cur: FieldInfo | null = p.mode === "edit" ? p.field : null;
  const kind: FieldKind = p.mode === "edit" ? p.field.kind : p.kind;
  const names = () => fieldList().map((f) => f.name);
  const [name, setName] = createSignal(cur?.name ?? freeName(kind, names(), locale()));
  const [tooltip, setTooltip] = createSignal(cur?.tooltip ?? "");
  const [required, setRequired] = createSignal(cur?.required ?? false);
  const [readOnly, setReadOnly] = createSignal(cur?.read_only ?? false);
  const [value, setValue] = createSignal(cur?.value ?? "");
  const [options, setOptions] = createSignal((cur?.options ?? []).join("\n"));
  const [exportValue, setExportValue] = createSignal(cur?.export_value ?? "");
  const hasOptions = kind === "combo_box" || kind === "list_box";
  const hasValue = kind === "text" || hasOptions;
  const err = () => { const k = nameError(name(), names(), cur?.name); return k ? ft(k) : undefined; };

  const done = () => { setSelectedField(null); props.onClose(); };
  const save = async () => {
    if (err()) return;
    const ok = p.mode === "new"
      ? await addField(p.doc, newSpec(kind, p.page, p.rect, name().trim(), {
        required: required(), read_only: readOnly(), tooltip: tooltip() || null, value: value() || null,
        ...(hasOptions ? { options: parseOptions(options()) } : {}),
        ...(kind === "radio_button" && exportValue() ? { export_value: exportValue() } : {}),
      }, locale()))
      : await updateField(p.doc, p.field.name, patchOf(p.field, { name: name().trim(), required: required(), read_only: readOnly(), tooltip: tooltip(), options: parseOptions(options()), value: value() }));
    if (ok) done();
  };
  const remove = async () => { if (p.mode === "edit" && await deleteField(p.doc, p.field.name)) done(); };

  return (
    <TaskPanel
      title={p.mode === "edit" ? ft("fdesign.panel.title", { name: p.field.name }) : ft("fdesign.panel.new")}
      onClose={done}
      primary={{ label: p.mode === "edit" ? ft("fdesign.panel.save") : ft("fdesign.panel.create"), disabled: !!err(), onClick: () => void save() }}
      secondary={p.mode === "edit" ? [{ label: ft("fdesign.panel.delete"), variant: "danger", onClick: () => void remove() }] : undefined}
      estimate={ft("fdesign.tool." + kind)}
      notice={<Show when={err() && name().trim() !== ""}><Notice tone="danger" inline>{err()}</Notice></Show>}
    >
      <Field label={ft("fdesign.panel.name")} error={err()} required><TextInput value={name()} onInput={(e) => setName(e.currentTarget.value)} /></Field>
      <Field label={ft("fdesign.panel.tooltip")}><TextInput value={tooltip()} onInput={(e) => setTooltip(e.currentTarget.value)} /></Field>
      <Show when={hasValue}><Field label={ft("fdesign.panel.value")}><TextInput value={value()} onInput={(e) => setValue(e.currentTarget.value)} /></Field></Show>
      <Show when={hasOptions}>
        <Field label={ft("fdesign.panel.options")}>
          <textarea class="pl-input" rows="4" value={options()} onInput={(e) => setOptions(e.currentTarget.value)} />
        </Field>
      </Show>
      <Show when={kind === "radio_button" && p.mode === "new"}>
        <Field label={ft("fdesign.panel.exportValue")}><TextInput value={exportValue()} placeholder={name()} onInput={(e) => setExportValue(e.currentTarget.value)} /></Field>
      </Show>
      <Checkbox label={ft("fdesign.panel.required")} checked={required()} onChange={(e) => setRequired(e.currentTarget.checked)} />
      <Checkbox label={ft("fdesign.panel.readonly")} checked={readOnly()} onChange={(e) => setReadOnly(e.currentTarget.checked)} />
    </TaskPanel>
  );
}
