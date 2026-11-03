import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { api, events } from "./api";
import { t } from "./i18n";
import {
  activeDoc, activeId, closeDoc, currentPageIndex, docs, listenForChanges, openPath, rotation, setActiveId, setRotation,
  setZoom, zoom,
} from "./state/documents";
import { fitPageZoom, fitWidthZoom, rotate, stepZoom } from "./viewer/layout";
import { Viewer } from "./viewer/Viewer";
import { Thumbnails } from "./panels/Thumbnails";
import { Outline } from "./panels/Outline";
import { SearchBox } from "./components/SearchBox";

export function App() {
  let main!: HTMLElement;
  const [panel, setPanel] = createSignal<"thumbs" | "outline" | null>("thumbs");
  const [error, setError] = createSignal<string | null>(null);
  const [passwordFor, setPasswordFor] = createSignal<string | null>(null);
  const [wrongPassword, setWrongPassword] = createSignal(false);

  const open = async (path: string, password?: string) => {
    const result = await openPath(path, main.clientWidth, password);
    if (result === "password") {
      setWrongPassword(password !== undefined);
      setPasswordFor(path);
    } else if (result !== "ok") setError(`${t("error.open")}: ${result}`);
    else { setPasswordFor(null); setError(null); }
  };

  onMount(async () => {
    const offs = await Promise.all([
      listenForChanges(),
      events.onOpenFiles((paths) => paths.forEach((p) => void open(p))),
      getCurrentWebview().onDragDropEvent((e) => {
        if (e.payload.type === "drop") e.payload.paths.filter((p) => /\.pdf$/i.test(p)).forEach((p) => void open(p));
      }),
    ]);
    onCleanup(() => offs.forEach((off) => off()));
    for (const p of await api.startupFiles()) await open(p);
  });

  const doc = activeDoc;
  const first = () => doc()?.sizes[0];

  return (
    <div class="shell">
      <header class="tabs" role="tablist">
        <For each={docs()}>{(d) => (
          <div class="tab" classList={{ active: d.id === activeId() }} role="tab" onClick={() => setActiveId(d.id)}>
            <span class="tab-title">{d.title}</span>
            <button class="icon-btn" title={t("tabs.close")} aria-label={t("tabs.close")} onClick={(e) => { e.stopPropagation(); void closeDoc(d.id); }}>×</button>
          </div>
        )}</For>
      </header>
      <Show when={doc()}>
        <div class="toolbar">
          <button title={t("toolbar.zoomOut")} aria-label={t("toolbar.zoomOut")} onClick={() => setZoom(stepZoom(zoom(), -1))}>−</button>
          <span class="zoom-label">{Math.round(zoom() * 100)} %</span>
          <button title={t("toolbar.zoomIn")} aria-label={t("toolbar.zoomIn")} onClick={() => setZoom(stepZoom(zoom(), 1))}>+</button>
          <button title={t("toolbar.fitWidth")} onClick={() => first() && setZoom(fitWidthZoom(first()!, rotation(), main.clientWidth))}>↔</button>
          <button title={t("toolbar.fitPage")} onClick={() => first() && setZoom(fitPageZoom(first()!, rotation(), { width: main.clientWidth, height: main.clientHeight }))}>▢</button>
          <button title={t("toolbar.rotate")} onClick={() => setRotation(rotate(rotation()))}>⟳</button>
          <span class="toolbar-sep" />
          <button classList={{ on: panel() === "thumbs" }} onClick={() => setPanel(panel() === "thumbs" ? null : "thumbs")}>{t("panel.thumbnails")}</button>
          <button classList={{ on: panel() === "outline" }} onClick={() => setPanel(panel() === "outline" ? null : "outline")}>{t("panel.outline")}</button>
          <SearchBox />
        </div>
      </Show>
      <main ref={main} classList={{ "with-panel": !!doc() && panel() !== null }}>
        <Show when={doc() && panel()}>
          <aside class="sidebar">
            <Show when={doc()} keyed>{(d) => panel() === "thumbs" ? <Thumbnails doc={d} /> : <Outline doc={d} />}</Show>
          </aside>
        </Show>
        <Show when={doc()} keyed fallback={<div class="empty"><h1>{t("empty.title")}</h1><p>{t("empty.hint")}</p></div>}>
          {(d) => <div class="viewer-host"><Viewer doc={d} zoom={zoom()} rotation={rotation()} /></div>}
        </Show>
        <Show when={error()}><div class="toast" role="alert">{error()}</div></Show>
        <Show when={passwordFor()}>
          {(path) => <PasswordDialog wrong={wrongPassword()} onSubmit={(pw) => void open(path(), pw)} onCancel={() => setPasswordFor(null)} />}
        </Show>
      </main>
      <footer class="status">
        <Show when={doc()}>{(d) => <span>{t("status.page", { page: currentPageIndex() + 1, total: d().sizes.length })}</span>}</Show>
      </footer>
    </div>
  );
}

function PasswordDialog(props: { wrong: boolean; onSubmit: (pw: string) => void; onCancel: () => void }) {
  const [pw, setPw] = createSignal("");
  return (
    <div class="modal-backdrop">
      <form class="modal" onSubmit={(e) => { e.preventDefault(); props.onSubmit(pw()); }}>
        <h2>{t("password.title")}</h2>
        <label>{t("password.label")}<input type="password" autofocus value={pw()} onInput={(e) => setPw(e.currentTarget.value)} /></label>
        <Show when={props.wrong}><p class="field-error">{t("password.wrong")}</p></Show>
        <div class="modal-actions">
          <button type="button" onClick={props.onCancel}>{t("password.cancel")}</button>
          <button type="submit" class="primary">{t("password.submit")}</button>
        </div>
      </form>
    </div>
  );
}
