import { createEffect, createMemo, createSignal, For, on, Show } from "solid-js";
import { locale } from "../i18n";
import { Button, Icon, IconButton, Kbd } from "../ui";
import { settings } from "../settings/store";
import { bestScore } from "./fuzzy";
import { displayCombo, eventCombo } from "./keys";
import { assignShortcut, effectiveKeys, resetShortcut, shortcutDefs, shortcutTitle, type ShortcutSection } from "./shortcuts";
import { nt, type NavKey } from "./strings";

const [open, setOpen] = createSignal(false);
export function openShortcutSheet(): void { setOpen(true); }
export const closeShortcutSheet = (): void => { setOpen(false); };
const SECTIONS: ShortcutSection[] = ["file", "navigation", "zoom", "search", "panels", "modes", "tools", "edit"];

/** Hoja de atajos (`F1`): todos los atajos por sección; los propios se pueden cambiar (sin conflictos). */
export function ShortcutSheet() {
  let dialog!: HTMLDialogElement;
  let filter!: HTMLInputElement;
  const [q, setQ] = createSignal("");
  const [capturing, setCapturing] = createSignal<string | null>(null);
  const [error, setError] = createSignal("");

  createEffect(on(open, (isOpen) => {
    if (isOpen && !dialog.open) { setQ(""); setCapturing(null); setError(""); dialog.showModal(); queueMicrotask(() => filter.focus()); }
    else if (!isOpen && dialog.open) dialog.close();
  }));

  const sections = createMemo(() => SECTIONS.map((s) => ({
    section: s,
    items: shortcutDefs().filter((d) => d.section === s && (!q().trim() || bestScore(q(), [shortcutTitle(d), effectiveKeys(d)]) !== null)),
  })).filter((s) => s.items.length > 0));

  const capture = (id: string) => (e: KeyboardEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape") { setCapturing(null); return; }
    const combo = eventCombo(e);
    if (!combo) return;
    const other = assignShortcut(id, e.key === "Backspace" ? "" : combo);
    if (other) setError(nt("nav.shortcuts.conflict", { title: other })); else { setError(""); setCapturing(null); }
  };

  return (
    <dialog ref={dialog} class="pl-palette pl-sheet" aria-label={nt("nav.shortcuts")} onCancel={(e) => { e.preventDefault(); if (capturing()) setCapturing(null); else setOpen(false); }} onClose={() => setOpen(false)}
      onClick={(e) => { if (e.target === dialog) setOpen(false); }}>
      <div class="pl-palette__search">
        <Icon name="keyboard" size={18} />
        <input ref={filter} type="search" aria-label={nt("nav.shortcuts.search")} placeholder={nt("nav.shortcuts.search")} value={q()} onInput={(e) => setQ(e.currentTarget.value)} />
        <Button variant="ghost" onClick={() => setOpen(false)}>{nt("nav.shortcuts.close")}</Button>
      </div>
      <div class="pl-sheet__body">
        <Show when={error()}><p class="pl-sheet__note" role="alert">{error()}</p></Show>
        <For each={sections()}>{(s) => (
          <section>
            <h3 class="pl-label-group">{nt(`nav.section.${s.section}` as NavKey)}</h3>
            <div class="pl-sheet__table">
              <For each={s.items}>{(d) => {
                const own = () => settings().shortcuts[d.id] !== undefined;
                return (
                  <div class="pl-sheet__row">
                    <span>{shortcutTitle(d)}<Show when={own()}> <span class="pl-sheet__note">· {nt("nav.shortcuts.customized")}</span></Show></span>
                    <span>
                      <Show when={capturing() === d.id} fallback={
                        <Show when={effectiveKeys(d)} fallback={<span class="pl-muted">{nt("nav.shortcuts.none")}</span>}><Kbd keys={displayCombo(effectiveKeys(d), locale())} /></Show>
                      }>
                        <input class="pl-sheet__capture" readOnly autofocus aria-label={nt("nav.shortcuts.press")} placeholder={nt("nav.shortcuts.press")} onKeyDown={capture(d.id)} onBlur={() => setCapturing(null)} />
                      </Show>
                    </span>
                    <span>
                      <Show when={d.run}>
                        <IconButton icon="pencil" label={nt("nav.shortcuts.edit", { title: shortcutTitle(d) })} onClick={() => { setError(""); setCapturing(d.id); }} />
                        <Show when={own()}><Button variant="ghost" onClick={() => resetShortcut(d.id)}>{nt("nav.shortcuts.reset")}</Button></Show>
                      </Show>
                    </span>
                  </div>
                );
              }}</For>
            </div>
          </section>
        )}</For>
      </div>
    </dialog>
  );
}
