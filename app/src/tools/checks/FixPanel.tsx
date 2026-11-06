import { withPassword } from "../../state/passwords";
import "./checks.css";
import { createEffect, createSignal, For, on, Show } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { locale } from "../../i18n";
import { activeDoc } from "../../state/documents";
import { goToPage } from "../../state/navigation";
import { Button, Checkbox, Field, Notice, Select, TaskPanel, TextInput } from "../../ui";
import { errorText, fileName, runToolJob } from "../run";
import { defaultLanguage, figuresWithoutAlt, fixOptions, LANGUAGES, type Figure } from "./fix";
import type { A11yReport } from "./report";
import { ct } from "./text";

const languageName = (tag: string): string => {
  try { return `${new Intl.DisplayNames([locale()], { type: "language" }).of(tag) ?? tag} (${tag})`; } catch { return tag; }
};

/** Hacer accesible: etiqueta el documento si no lo está y pone título, idioma, textos alternativos y tabulación.
 *  Guarda una copia; el original no cambia. */
export default function FixPanel(p: { onClose: () => void }) {
  const [report, setReport] = createSignal<A11yReport | null>(null);
  const [error, setError] = createSignal<string | null>(null);
  const [title, setTitle] = createSignal("");
  const [language, setLanguage] = createSignal("");
  const [autotag, setAutotag] = createSignal(true);
  const [tabOrder, setTabOrder] = createSignal(true);
  const [alts, setAlts] = createSignal<Record<number, string>>({});
  const [figures, setFigures] = createSignal<Figure[]>([]);

  createEffect(on(() => activeDoc()?.path, async (path) => {
    setReport(null); setError(null); setAlts({});
    if (!path) return;
    try {
      const r = await invoke<A11yReport>("check_accessibility", withPassword({ path }));
      setReport(r);
      setTitle(r.title ?? fileName(path).replace(/\.pdf$/i, ""));
      setLanguage(defaultLanguage(r, locale()));
      setFigures(figuresWithoutAlt(r));
    } catch (e) {
      setError(ct("checks.failed", { error: errorText(e) }));
    }
  }));

  const options = () => {
    const r = report();
    return r ? fixOptions({ tagged: r.tagged, autotag: autotag(), title: title(), language: language(), tabOrder: tabOrder(), alts: alts() }, r) : null;
  };
  const languages = () => {
    const cur = report()?.language;
    return (cur && !LANGUAGES.includes(cur) ? [cur, ...LANGUAGES] : LANGUAGES).map((v) => ({ value: v, label: languageName(v) }));
  };

  const run = async () => {
    const d = activeDoc();
    const o = options();
    if (!d || !o) return;
    const id = await runToolJob({
      command: "make_accessible", args: { path: d.path, options: o, suffix: ct("checks.fix.suffix") },
      title: ct("checks.fix.job", { name: fileName(d.path) }), started: ct("checks.fix.started"),
    });
    if (id !== null) p.onClose();
  };

  return (
    <TaskPanel
      title={ct("checks.fix.title")} onClose={p.onClose} estimate={ct("checks.fix.hint")}
      primary={{ label: ct("checks.fix.run"), icon: "wand", disabled: options() === null, onClick: () => void run() }}
      notice={<Show when={error()}><Notice tone="danger" inline>{error()}</Notice></Show>}
    >
      <Show when={report()} fallback={<Show when={!error()}><p class="muted">{ct("checks.running")}</p></Show>}>
        {(r) => (
          <>
            <Show when={!r().tagged} fallback={<Notice tone="ok" inline>{ct("checks.fix.tagged")}</Notice>}>
              <Checkbox label={ct("checks.fix.autotag")} checked={autotag()} onChange={(e) => setAutotag(e.currentTarget.checked)} />
            </Show>
            <Field label={ct("checks.fix.docTitle")} hint={ct("checks.fix.docTitleHint")}>
              <TextInput value={title()} onInput={(e) => setTitle(e.currentTarget.value)} />
            </Field>
            <Field label={ct("checks.fix.language")}>
              <Select value={language()} onChange={(e) => setLanguage(e.currentTarget.value)} options={languages()} />
            </Field>
            <Checkbox label={ct("checks.fix.tabOrder")} checked={tabOrder()} disabled={!r().tagged && !autotag()}
              onChange={(e) => setTabOrder(e.currentTarget.checked)} />
            <Show when={figures().length} fallback={<Show when={!r().tagged}><p class="muted">{ct("checks.fix.altLater")}</p></Show>}>
              <h3 class="pl-checks-subtitle">{ct("checks.fix.alts", { n: figures().length })}</h3>
              <For each={figures()}>
                {(f, i) => (
                  <Field label={f.page !== null ? ct("checks.fix.figurePage", { n: i() + 1, page: f.page + 1 }) : ct("checks.fix.figure", { n: i() + 1 })}>
                    <div class="pl-checks-alt">
                      <TextInput value={alts()[f.xref] ?? ""} placeholder={ct("checks.fix.altPlaceholder")}
                        onInput={(e) => setAlts({ ...alts(), [f.xref]: e.currentTarget.value })} />
                      <Show when={f.page !== null}>
                        <Button variant="ghost" onClick={() => goToPage(f.page!)}>{ct("checks.fix.show")}</Button>
                      </Show>
                    </div>
                  </Field>
                )}
              </For>
            </Show>
          </>
        )}
      </Show>
    </TaskPanel>
  );
}
