import { createContext, createUniqueId, For, Show, splitProps, useContext, type JSX } from "solid-js";
import "./Field.css";

interface FieldCtx { id: string; describedBy: string | undefined; invalid: boolean; required: boolean }
const Ctx = createContext<FieldCtx>();

/** Etiqueta + control + ayuda/error enlazados con ARIA. TextInput, Select y Slider leen su contexto. */
export function Field(props: { label: string; hint?: string; error?: string; required?: boolean; children: JSX.Element; class?: string }) {
  const id = createUniqueId();
  const describedBy = () => [props.hint ? `${id}-hint` : "", props.error ? `${id}-err` : ""].filter(Boolean).join(" ") || undefined;
  return (
    <Ctx.Provider value={{
      id, get describedBy() { return describedBy(); }, get invalid() { return !!props.error; }, get required() { return !!props.required },
    }}>
      <div class={`pl-field ${props.class ?? ""}`}>
        <label class="pl-field__label" for={id}>{props.label}{props.required && <span aria-hidden="true"> *</span>}</label>
        {props.children}
        <Show when={props.hint}><p class="pl-field__hint" id={`${id}-hint`}>{props.hint}</p></Show>
        <Show when={props.error}><p class="pl-field__error" id={`${id}-err`} role="alert">{props.error}</p></Show>
      </div>
    </Ctx.Provider>
  );
}

export function TextInput(props: JSX.InputHTMLAttributes<HTMLInputElement>) {
  const f = useContext(Ctx);
  const [own, rest] = splitProps(props, ["class"]);
  return (
    <input
      class={`pl-input ${own.class ?? ""}`} id={f?.id} aria-describedby={f?.describedBy} aria-invalid={f?.invalid || undefined}
      aria-required={f?.required || undefined} {...rest}
    />
  );
}

export interface SelectOption { value: string; label: string; disabled?: boolean }

export function Select(props: Omit<JSX.SelectHTMLAttributes<HTMLSelectElement>, "children"> & { options: SelectOption[] }) {
  const f = useContext(Ctx);
  const [own, rest] = splitProps(props, ["options", "class", "value"]);
  return (
    <select class={`pl-input pl-select ${own.class ?? ""}`} id={f?.id} aria-describedby={f?.describedBy} aria-invalid={f?.invalid || undefined} {...rest}>
      <For each={own.options}>{(o) => <option value={o.value} disabled={o.disabled} selected={o.value === own.value}>{o.label}</option>}</For>
    </select>
  );
}

export function Slider(props: Omit<JSX.InputHTMLAttributes<HTMLInputElement>, "type"> & { valueLabel?: string }) {
  const f = useContext(Ctx);
  const [own, rest] = splitProps(props, ["class", "valueLabel"]);
  return (
    <div class={`pl-slider ${own.class ?? ""}`}>
      <input type="range" class="pl-slider__input" id={f?.id} aria-describedby={f?.describedBy} aria-valuetext={own.valueLabel} {...rest} />
      <Show when={own.valueLabel}><span class="pl-slider__value pl-num" aria-hidden="true">{own.valueLabel}</span></Show>
    </div>
  );
}
