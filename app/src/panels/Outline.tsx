import { createResource, For, Show } from "solid-js";
import { api, type OutlineItem } from "../api";
import { t } from "../i18n";
import { goToPage } from "../state/navigation";
import type { OpenDoc } from "../state/documents";

function Node(props: { item: OutlineItem }) {
  return (
    <li>
      <button class="outline-item" disabled={props.item.page === null} onClick={() => props.item.page !== null && goToPage(props.item.page)}>
        {props.item.title}
      </button>
      <Show when={props.item.children.length}>
        <ul><For each={props.item.children}>{(c) => <Node item={c} />}</For></ul>
      </Show>
    </li>
  );
}

export function Outline(props: { doc: OpenDoc }) {
  const [items] = createResource(() => props.doc.id, (id) => api.outline(id).catch(() => [] as OutlineItem[]));
  return (
    <div class="outline" data-testid="outline">
      <Show when={items()?.length} fallback={<p class="muted">{t("panel.noOutline")}</p>}>
        <ul><For each={items()}>{(i) => <Node item={i} />}</For></ul>
      </Show>
    </div>
  );
}
