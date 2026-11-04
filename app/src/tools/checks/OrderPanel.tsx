import "./checks.css";
import { createEffect, createSignal, For, on, Show } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { errorText, fileName, runToolJob } from "../run";
import { activeDoc } from "../../state/documents";
import { Notice, TaskPanel } from "../../ui";
import { ct } from "./text";
import { backwardsPages, orderRows, type ReadingItem } from "./order";

/** Orden de lectura: muestra el orden de la estructura y puede guardar una copia con la tabulación ajustada a ella. */
export default function OrderPanel(p: { onClose: () => void }) {
  const [items, setItems] = createSignal<ReadingItem[] | null | undefined>(undefined);
  const [error, setError] = createSignal<string | null>(null);

  createEffect(on(() => activeDoc()?.path, async (path) => {
    setItems(undefined); setError(null);
    if (!path) return;
    try { setItems(await invoke<ReadingItem[] | null>("reading_order", { path })); }
    catch (e) { setError(ct("checks.failed", { error: errorText(e) })); }
  }));

  const fix = async () => {
    const d = activeDoc();
    if (!d) return;
    const id = await runToolJob({
      command: "tab_order_copy", args: { path: d.path, suffix: ct("checks.order.suffix") },
      title: ct("checks.order.job", { name: fileName(d.path) }), started: ct("checks.order.started"),
    });
    if (id !== null) p.onClose();
  };

  return (
    <TaskPanel
      title={ct("checks.order.title")} onClose={p.onClose} estimate={ct("checks.order.hint")}
      primary={{ label: ct("checks.order.fix"), icon: "list-ordered", disabled: !activeDoc() || !items(), onClick: () => void fix() }}
      notice={<Show when={error()}><Notice tone="danger" inline>{error()}</Notice></Show>}
    >
      <Show when={items() === null}><Notice tone="warn" inline>{ct("checks.order.untagged")}</Notice></Show>
      <Show when={items()}>
        {(list) => (
          <>
            <Show when={backwardsPages(list()).length}>
              <Notice tone="warn" inline>{ct("checks.order.backwards", { pages: backwardsPages(list()).join(", ") })}</Notice>
            </Show>
            <ol class="pl-checks-list">
              <For each={orderRows(list())}>
                {(row) => (
                  <li data-tone={row.noAlt ? "warn" : "ok"}>
                    <Show when={row.page !== null}><strong>{ct("checks.page", { n: (row.page ?? 0) + 1 })}. </strong></Show>
                    {row.role}
                    <Show when={row.noAlt}><small>{ct("checks.order.noAlt")}</small></Show>
                  </li>
                )}
              </For>
            </ol>
          </>
        )}
      </Show>
    </TaskPanel>
  );
}
