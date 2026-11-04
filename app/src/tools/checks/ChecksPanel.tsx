import "./checks.css";
import { createSignal, For, Show } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { errorText } from "../run";
import { activeDoc } from "../../state/documents";
import { Field, Notice, Select, TaskPanel } from "../../ui";
import { ct } from "./text";
import { fromA11y, fromPdfa, fromPreflight, type A11yReport, type PdfaValidation, type PreflightReport, type Summary } from "./report";

type Kind = "access.check" | "access.validate-pdfa" | "print.preflight";
const titles: Record<Kind, string> = { "access.check": "checks.access.title", "access.validate-pdfa": "checks.pdfa.title", "print.preflight": "checks.preflight.title" };

/** Panel común de las tres comprobaciones de solo lectura: accesibilidad, PDF/A y comprobación previa. */
export default function ChecksPanel(p: { toolId: Kind; onClose: () => void }) {
  const [level, setLevel] = createSignal("2b");
  const [profile, setProfile] = createSignal("print");
  const [busy, setBusy] = createSignal(false);
  const [result, setResult] = createSignal<Summary | null>(null);
  const [error, setError] = createSignal<string | null>(null);

  const run = async () => {
    const doc = activeDoc();
    if (!doc) return;
    setBusy(true); setError(null); setResult(null);
    try {
      if (p.toolId === "access.check") setResult(fromA11y(await invoke<A11yReport>("check_accessibility", { path: doc.path })));
      else if (p.toolId === "access.validate-pdfa") setResult(fromPdfa(await invoke<PdfaValidation>("validate_pdfa", { path: doc.path, level: level() })));
      else setResult(fromPreflight(await invoke<PreflightReport>("preflight_pdf", { path: doc.path, profile: profile() })));
    } catch (e) {
      setError(ct("checks.failed", { error: errorText(e) }));
    } finally {
      setBusy(false);
    }
  };

  return (
    <TaskPanel
      title={ct(titles[p.toolId])} onClose={p.onClose}
      primary={{ label: busy() ? ct("checks.running") : ct("checks.run"), icon: "check-circle", disabled: busy(), onClick: () => void run() }}
      estimate={ct("checks.hint")}
      notice={<Show when={error()}><Notice tone="danger" inline>{error()}</Notice></Show>}
    >
      <Show when={p.toolId === "access.validate-pdfa"}>
        <Field label={ct("checks.pdfa.level")}>
          <Select value={level()} onChange={(e) => setLevel(e.currentTarget.value)} options={["1b", "2b", "2u", "3b"].map((v) => ({ value: v, label: `PDF/A-${v}` }))} />
        </Field>
      </Show>
      <Show when={p.toolId === "print.preflight"}>
        <Field label={ct("checks.preflight.profile")}>
          <Select value={profile()} onChange={(e) => setProfile(e.currentTarget.value)} options={["basic", "print", "pdfx"].map((v) => ({ value: v, label: ct(`checks.profile.${v}`) }))} />
        </Field>
      </Show>
      <Show when={result()}>
        {(r) => (
          <>
            <Notice tone={r().errors ? "danger" : r().warnings ? "warn" : "ok"} inline>
              {r().rows.length ? ct("checks.result", { errors: r().errors, warnings: r().warnings }) : ct("checks.ok")}
            </Notice>
            <ul class="pl-checks-list">
              <For each={r().rows}>
                {(row) => (
                  <li data-tone={row.tone}>
                    <Show when={row.page !== null}><strong>{ct("checks.page", { n: (row.page ?? 0) + 1 })}. </strong></Show>
                    {row.text}
                    <Show when={row.hint}><small>{row.hint}</small></Show>
                  </li>
                )}
              </For>
            </ul>
          </>
        )}
      </Show>
    </TaskPanel>
  );
}
