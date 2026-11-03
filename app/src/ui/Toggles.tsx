import { createContext, createUniqueId, For, splitProps, useContext, type JSX } from "solid-js";
import { Icon } from "./Icon";
import "./Toggles.css";

/** Checkbox con etiqueta. Admite `indeterminate`. */
export function Checkbox(props: Omit<JSX.InputHTMLAttributes<HTMLInputElement>, "type" | "children"> & { label: string; indeterminate?: boolean; hint?: string }) {
  const [own, rest] = splitProps(props, ["label", "hint", "indeterminate", "class"]);
  const id = createUniqueId();
  return (
    <div class={`pl-choice ${own.class ?? ""}`}>
      <input type="checkbox" class="pl-choice__input pl-checkbox" id={id} aria-describedby={own.hint ? `${id}-h` : undefined}
        ref={(el) => queueMicrotask(() => { el.indeterminate = !!own.indeterminate; })} {...rest} />
      <label for={id}>{own.label}</label>
      {own.hint && <p class="pl-choice__hint" id={`${id}-h`}>{own.hint}</p>}
    </div>
  );
}

/** Interruptor (role="switch"): efecto inmediato, sin botón Guardar. */
export function Switch(props: Omit<JSX.InputHTMLAttributes<HTMLInputElement>, "type" | "children"> & { label: string; hint?: string }) {
  const [own, rest] = splitProps(props, ["label", "hint", "class"]);
  const id = createUniqueId();
  return (
    <div class={`pl-choice ${own.class ?? ""}`}>
      <input type="checkbox" role="switch" class="pl-choice__input pl-switch" id={id} aria-describedby={own.hint ? `${id}-h` : undefined} {...rest} />
      <label for={id}>{own.label}</label>
      {own.hint && <p class="pl-choice__hint" id={`${id}-h`}>{own.hint}</p>}
    </div>
  );
}

interface RadioCtx { name: string; value: () => string | undefined; set: (v: string) => void }
const RadioContext = createContext<RadioCtx>();

/** Grupo de radios (role="radiogroup"); las flechas mueven la selección de forma nativa. */
export function RadioGroup(props: { legend: string; value?: string; onChange?: (v: string) => void; children: JSX.Element; class?: string }) {
  const name = createUniqueId();
  return (
    <RadioContext.Provider value={{ name, value: () => props.value, set: (v) => props.onChange?.(v) }}>
      <fieldset class={`pl-radiogroup ${props.class ?? ""}`} role="radiogroup">
        <legend class="pl-field__label">{props.legend}</legend>
        {props.children}
      </fieldset>
    </RadioContext.Provider>
  );
}

export function Radio(props: { value: string; label: string; hint?: string; disabled?: boolean }) {
  const g = useContext(RadioContext)!;
  const id = createUniqueId();
  return (
    <div class="pl-choice">
      <input type="radio" class="pl-choice__input pl-radio" id={id} name={g.name} value={props.value} disabled={props.disabled}
        checked={g.value() === props.value} aria-describedby={props.hint ? `${id}-h` : undefined} onChange={() => g.set(props.value)} />
      <label for={id}>{props.label}</label>
      {props.hint && <p class="pl-choice__hint" id={`${id}-h`}>{props.hint}</p>}
    </div>
  );
}

export interface ToggleOption { value: string; label?: string; icon?: string; title?: string }

/** Grupo de botones conmutables exclusivos (p. ej. vista continua / página / doble). */
export function ToggleGroup(props: { label: string; options: ToggleOption[]; value: string; onChange: (v: string) => void; class?: string }) {
  return (
    <div class={`pl-togglegroup ${props.class ?? ""}`} role="radiogroup" aria-label={props.label}>
      <For each={props.options}>{(o) => (
        <ToggleItem option={o} active={o.value === props.value} onSelect={() => props.onChange(o.value)} />
      )}</For>
    </div>
  );
}
function ToggleItem(props: { option: ToggleOption; active: boolean; onSelect: () => void }) {
  return (
    <button type="button" role="radio" aria-checked={props.active} aria-label={props.option.title ?? props.option.label} title={props.option.title ?? props.option.label}
      tabindex={props.active ? 0 : -1} class="pl-togglegroup__item" onClick={() => props.onSelect()}
      onKeyDown={(e) => {
        const dir = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 0;
        if (!dir) return;
        e.preventDefault();
        const items = Array.from(e.currentTarget.parentElement!.querySelectorAll<HTMLButtonElement>("button"));
        const next = items[(items.indexOf(e.currentTarget) + dir + items.length) % items.length]!;
        next.focus(); next.click();
      }}>
      {props.option.icon && <Icon name={props.option.icon} size={16} />}
      {props.option.label}
    </button>
  );
}
