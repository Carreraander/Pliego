import { Show } from "solid-js";
import { t } from "../i18n";
import { clearSearch, matchIndex, matches, nextMatch, query, runSearch, searched } from "../state/search";
import { activeId } from "../state/documents";

export function SearchBox() {
  const submit = (text: string) => {
    const id = activeId();
    if (id !== null) void runSearch(id, text);
  };
  return (
    <div class="searchbox">
      <input type="search" placeholder={t("search.placeholder")} aria-label={t("toolbar.search")} value={query()}
        onKeyDown={(e) => {
          if (e.key === "Enter") (query() === e.currentTarget.value && matches().length ? nextMatch(e.shiftKey ? -1 : 1) : submit(e.currentTarget.value));
          if (e.key === "Escape") clearSearch();
        }}
        onInput={(e) => { if (!e.currentTarget.value) clearSearch(); }} />
      <Show when={searched()}>
        <span class="muted">{matches().length ? t("search.count", { current: matchIndex() + 1, total: matches().length }) : t("search.none")}</span>
        <button onClick={() => nextMatch(-1)} aria-label="‹" disabled={!matches().length}>‹</button>
        <button onClick={() => nextMatch(1)} aria-label="›" disabled={!matches().length}>›</button>
      </Show>
    </div>
  );
}
