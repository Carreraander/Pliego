import "./forms.css";
import { createSignal, For, Show } from "solid-js";
import { activeMode } from "../../shell";
import type { PageOverlay } from "../../viewer/overlays";
import { addComment } from "../annotate/edit";
import type { OpenDoc } from "../../state/documents";
import { fillField, filterKeystroke, runFieldAction } from "./fill";
import { applyKeystroke, describeEdit } from "./keystroke";
import { placeSignature } from "./signature";
import { fieldsOnPage, placingSignature, setPlacingSignature, signature, type FormField } from "./state";
import { ft } from "./text";

const SIGNATURE_WIDTH = 150;

/** Campo de texto. Sin enfocar enseña el texto con el formato del campo (`1.234,50 €`); al teclear enseña el valor y pasa cada pulsación por su script de tecleo (que puede rechazarla o cambiarla). */
function TextControl(p: { doc: OpenDoc; field: FormField; disabled: boolean; commit: (v: string) => void }) {
  const f = () => p.field;
  const [focused, setFocused] = createSignal(false);
  let prev = "";
  const shown = () => (focused() ? f().value ?? "" : f().display ?? f().value ?? "");
  const filtered = () => f().scripts?.includes("K") ?? false;

  const onInput = async (el: HTMLInputElement | HTMLTextAreaElement) => {
    const typed = el.value;
    const edit = filtered() ? describeEdit(prev, typed) : null;
    if (!edit) { prev = typed; return; }
    const r = await filterKeystroke(p.doc, f().name, edit.change, prev, edit.start, edit.end);
    if (el.value !== typed) { prev = el.value; return; } // se siguió tecleando mientras se esperaba
    const { value, caret } = applyKeystroke(prev, edit, r);
    if (value !== typed) { el.value = value; el.setSelectionRange(caret, caret); }
    prev = value;
  };
  const common = {
    class: "pl-field-ctl",
    get disabled() { return p.disabled; },
    get "aria-label"() { return f().name; },
    onFocus: (e: FocusEvent & { currentTarget: HTMLInputElement | HTMLTextAreaElement }) => { prev = f().value ?? ""; setFocused(true); e.currentTarget.value = prev; },
    onBlur: () => setFocused(false),
    onInput: (e: InputEvent & { currentTarget: HTMLInputElement | HTMLTextAreaElement }) => void onInput(e.currentTarget),
    onChange: (e: Event & { currentTarget: HTMLInputElement | HTMLTextAreaElement }) => p.commit(e.currentTarget.value),
  };
  return (
    <Show when={f().multiline} fallback={<input {...common} type={f().password ? "password" : "text"} value={shown()} />}>
      <textarea {...common} value={shown()} />
    </Show>
  );
}

function Control(p: { doc: OpenDoc; field: FormField; commit: (v: string | boolean) => void; act: () => void }) {
  const f = () => p.field;
  const off = (v: string | null) => v === null || v === "" || v === "Off";
  const disabled = () => f().read_only || f().signed;
  return (
    <>
      <Show when={f().kind === "text"}>
        <TextControl doc={p.doc} field={f()} disabled={disabled()} commit={(v) => p.commit(v)} />
      </Show>
      <Show when={f().kind === "push_button" && (f().scripts?.some((k) => k === "U" || k === "A") ?? false)}>
        <button type="button" class="pl-field-ctl pl-field-ctl--button" disabled={disabled()} aria-label={f().name} onClick={() => p.act()} />
      </Show>
      <Show when={f().kind === "check_box"}>
        <input class="pl-field-ctl pl-field-ctl--check" type="checkbox" checked={!off(f().value)} disabled={disabled()} aria-label={ft("forms.field.checkbox", { name: f().name })}
          onChange={(e) => p.commit(e.currentTarget.checked)} />
      </Show>
      <Show when={f().kind === "combo_box" || f().kind === "list_box" || f().kind === "radio_button"}>
        <select class="pl-field-ctl" disabled={disabled()} aria-label={f().name} value={f().value ?? ""} onChange={(e) => e.currentTarget.value && p.commit(e.currentTarget.value)}>
          <option value="">{ft("forms.field.option")}</option>
          <For each={f().options}>{(o) => <option value={o} selected={o === f().value}>{o}</option>}</For>
        </select>
      </Show>
    </>
  );
}

/** Capa de una página en el modo Rellenar y firmar: los controles de cada campo sobre su recuadro y la colocación de la firma. */
export const FieldsLayer: PageOverlay = (p) => {
  const active = () => activeMode()?.id === "forms.fill";
  const [busy, setBusy] = createSignal(false);
  const commit = async (f: FormField, v: string | boolean) => { setBusy(true); await fillField(p.doc, f.name, v); setBusy(false); };
  const act = async (f: FormField) => { setBusy(true); await runFieldAction(p.doc, f.name); setBusy(false); };

  const place = async (e: PointerEvent) => {
    const sig = signature();
    if (!sig || e.button !== 0) return;
    setPlacingSignature(false);
    const strokes = placeSignature(sig, [e.offsetX / p.zoom, e.offsetY / p.zoom], SIGNATURE_WIDTH);
    await addComment(p.doc, { page: p.index, geometry: { type: "ink", strokes }, style: { color: [0, 0, 0], border_width: 1.5 }, subject: "firma" });
  };

  return (
    <Show when={active()}>
      <div class="pl-fields-layer" data-busy={busy()} data-testid="fields-layer">
        <For each={fieldsOnPage(p.doc, p.index)}>{(f) => {
          const r = f.rect!;
          return (
            <div class="pl-field-box" style={{ left: `${r[0] * p.zoom}px`, top: `${r[1] * p.zoom}px`, width: `${(r[2] - r[0]) * p.zoom}px`, height: `${(r[3] - r[1]) * p.zoom}px` }}>
              <Control doc={p.doc} field={f} commit={(v) => void commit(f, v)} act={() => void act(f)} />
            </div>
          );
        }}</For>
        <Show when={placingSignature()}>
          <div class="pl-signature-target" role="presentation" onPointerDown={(e) => { if (e.target === e.currentTarget) void place(e); }} />
        </Show>
      </div>
    </Show>
  );
};
