import "./annotate.css";
import { createResource, For, Show } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { isApiError } from "../../api";
import { goToPage } from "../../state/navigation";
import { activeDoc } from "../../state/documents";
import { EmptyState, Notice, TaskPanel } from "../../ui";
import { at } from "./text";
import { withPassword } from "../../state/passwords";

/** Reflejo de `pliego_annot::CommentSummary` (solo lo que se muestra). */
export interface AnnotInfo { id: { page: number; xref: number }; kind: string; author?: string | null; contents?: string | null }
export interface Thread { comment: AnnotInfo; replies: AnnotInfo[] }
export interface CommentSummary { total: number; by_kind: Record<string, number>; by_author: Record<string, number>; threads: Thread[] }

export default function SummaryPanel(p: { onClose: () => void }) {
  const [data] = createResource(
    () => activeDoc()?.path,
    (path) => invoke<CommentSummary>("annot_summary", withPassword({ path })).catch((e) => { throw new Error(isApiError(e) ? e.message : String(e)); }),
  );
  const who = (a: AnnotInfo) => a.author || at("annotate.summary.noAuthor");

  return (
    <TaskPanel title={at("annotate.summary.title")} onClose={p.onClose} primary={{ label: at("annotate.summary.close"), variant: "normal", onClick: p.onClose }}
      estimate={data() ? at("annotate.summary.total", { n: data()!.total }) : undefined}
      notice={<Show when={data.error}><Notice tone="danger" inline>{String(data.error?.message ?? data.error)}</Notice></Show>}>
      <Show when={!data.loading} fallback={<p>{at("annotate.summary.loading")}</p>}>
        <Show when={data()?.threads.length} fallback={<EmptyState title={at("annotate.summary.none")} />}>
          <ul class="pl-comments">
            <For each={data()!.threads}>{(t) => (
              <li>
                <button type="button" class="pl-comments__item" onClick={() => goToPage(t.comment.id.page)}>
                  <span class="pl-comments__meta">{at("annotate.summary.page", { n: t.comment.id.page + 1 })} · {t.comment.kind} · {who(t.comment)}</span>
                  <span>{t.comment.contents || at("annotate.summary.empty")}</span>
                </button>
                <For each={t.replies}>{(r) => (
                  <p class="pl-comments__reply"><span class="pl-comments__meta">{who(r)}</span> {r.contents}</p>
                )}</For>
              </li>
            )}</For>
          </ul>
        </Show>
      </Show>
    </TaskPanel>
  );
}
