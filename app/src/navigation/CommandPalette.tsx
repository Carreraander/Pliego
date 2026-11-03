import { createEffect, createMemo, createSignal, createUniqueId, For, on, Show } from "solid-js";
import { Icon, Kbd } from "../ui";
import { searchCommands, type Command } from "./commands";
import { nt } from "./strings";

const [open, setOpen] = createSignal(false);
const [initial, setInitial] = createSignal("");
export function openCommandPalette(query = ""): void { setInitial(query); setOpen(true); }
export const closeCommandPalette = (): void => { setOpen(false); };

/** Paleta de órdenes (`Ctrl+K`): campo único, resultados agrupados, `↑↓` mover, `Intro` ejecutar, `Tab` ampliar, `Esc` cerrar. */
export function CommandPalette() {
  let dialog!: HTMLDialogElement;
  let input!: HTMLInputElement;
  const id = createUniqueId();
  const [q, setQ] = createSignal("");
  const [index, setIndex] = createSignal(0);
  const groups = createMemo(() => (open() ? searchCommands(q()) : []));
  const flat = createMemo(() => groups().flatMap((g) => g.items));
  const optionId = (i: number) => `${id}-opt-${i}`;

  createEffect(on(open, (isOpen) => {
    if (isOpen && !dialog.open) { setQ(initial()); setIndex(0); dialog.showModal(); queueMicrotask(() => { input.focus(); input.select(); }); }
    else if (!isOpen && dialog.open) dialog.close();
  }));
  createEffect(on(q, () => setIndex(0)));
  createEffect(() => { document.getElementById(optionId(index()))?.scrollIntoView({ block: "nearest" }); });

  const run = (c: Command | undefined) => {
    if (!c) return;
    if (c.disabledReason) return;
    setOpen(false);
    queueMicrotask(c.run);
  };
  const onKeyDown = (e: KeyboardEvent) => {
    const n = flat().length;
    if (e.key === "ArrowDown") { e.preventDefault(); setIndex((i) => (n ? (i + 1) % n : 0)); }
    else if (e.key === "ArrowUp") { e.preventDefault(); setIndex((i) => (n ? (i - 1 + n) % n : 0)); }
    else if (e.key === "Enter") { e.preventDefault(); run(flat()[index()]); }
    else if (e.key === "Tab") {
      e.preventDefault();
      const c = flat()[index()];
      if (c?.expand && !c.disabledReason) { setOpen(false); queueMicrotask(c.expand); } else if (c) setQ(c.title);
    }
  };

  return (
    <dialog ref={dialog} class="pl-palette" aria-label={nt("nav.palette")} onCancel={(e) => { e.preventDefault(); setOpen(false); }} onClose={() => setOpen(false)}
      onClick={(e) => { if (e.target === dialog) setOpen(false); }}>
      <div class="pl-palette__search">
        <Icon name="search" size={18} />
        <input ref={input} type="text" role="combobox" aria-expanded="true" aria-controls={`${id}-list`} aria-autocomplete="list" aria-activedescendant={flat().length ? optionId(index()) : undefined}
          aria-label={nt("nav.palette")} placeholder={nt("nav.palette.placeholder")} autocomplete="off" spellcheck={false} value={q()} onInput={(e) => setQ(e.currentTarget.value)} onKeyDown={onKeyDown} />
        <Kbd keys="Esc" />
      </div>
      <div class="pl-palette__list" id={`${id}-list`} role="listbox" aria-label={nt("nav.palette")}>
        <For each={groups()} fallback={<div class="pl-palette__empty" role="status">{nt("nav.palette.none", { q: q() })}</div>}>{(g) => (
          <>
            <div class="pl-palette__group pl-label-group" role="presentation">{g.label}</div>
            <For each={g.items}>{(c) => {
              const i = () => flat().indexOf(c);
              return (
                <div id={optionId(i())} role="option" class="pl-palette__item" aria-selected={i() === index()} aria-disabled={c.disabledReason ? true : undefined}
                  onMouseMove={() => setIndex(i())} onClick={() => run(c)}>
                  <Icon name={c.icon ?? "chevron-right"} size={18} />
                  <span>
                    <span class="pl-palette__title">{c.title}</span>
                    <Show when={c.disabledReason ?? c.subtitle}><span class="pl-palette__sub">{c.disabledReason ?? c.subtitle}</span></Show>
                  </span>
                  <Show when={c.shortcut}><Kbd keys={c.shortcut!} /></Show>
                </div>
              );
            }}</For>
          </>
        )}</For>
      </div>
      <div class="pl-palette__foot" aria-hidden="true">{nt("nav.palette.hint")}</div>
    </dialog>
  );
}
