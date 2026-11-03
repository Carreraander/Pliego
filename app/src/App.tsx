import { createSignal, onCleanup, onMount, Show } from "solid-js";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { api, events } from "./api";
import { t } from "./i18n";
import {
  activeDoc, activeId, closeDoc, currentPageIndex, docs, listenForChanges, openPath, rotation, setActiveId, setDocs, setRotation, setZoom, zoom,
} from "./state/documents";
import { fitPageZoom, fitWidthZoom, rotate, stepZoom } from "./viewer/layout";
import { Viewer } from "./viewer/Viewer";
import { Thumbnails } from "./panels/Thumbnails";
import { Outline } from "./panels/Outline";
import { SearchBox } from "./components/SearchBox";
import { installNavigation, NavigationHost, currentScreen } from "./navigation";
import { Dynamic } from "solid-js/web";
import { AppWindow, registerMode, registerPanel, registerStatusItem, setActivePanel, setMode } from "./shell";
import { Button, Dialog, EmptyState, Field, IconButton, TextInput, toast, ToolbarSeparator } from "./ui";

export function App() {
  let canvas!: HTMLDivElement;
  const [passwordFor, setPasswordFor] = createSignal<string | null>(null);
  const [wrongPassword, setWrongPassword] = createSignal(false);
  const [password, setPassword] = createSignal("");

  const open = async (path: string, pw?: string) => {
    const result = await openPath(path, canvas.clientWidth, pw);
    if (result === "password") {
      setWrongPassword(pw !== undefined);
      setPasswordFor(path);
    } else if (result !== "ok") toast({ tone: "danger", text: `${t("error.open")}: ${result}` });
    else { setPasswordFor(null); setPassword(""); }
  };

  const first = () => activeDoc()?.sizes[0];

  onMount(async () => {
    // Raíl, modo «Leer» y barra de estado del visor: se montan en las ranuras del armazón (docs/app-integration.md).
    const offs = [
      registerPanel({ id: "viewer.thumbnails", title: t("panel.thumbnails"), icon: "layout-grid", order: 10, shortcut: "F4",
        component: (p) => <Show when={docs().find((d) => String(d.id) === p.docId)} keyed>{(d) => <Thumbnails doc={d} />}</Show> }),
      registerPanel({ id: "viewer.outline", title: t("panel.outline"), icon: "bookmark", order: 20,
        component: (p) => <Show when={docs().find((d) => String(d.id) === p.docId)} keyed>{(d) => <Outline doc={d} />}</Show> }),
      registerMode({ id: "read", title: "Leer", icon: "mouse-pointer", order: 10, toolbar: () => (
        <>
          <IconButton icon="zoom-out" label={t("toolbar.zoomOut")} disabled={!activeDoc()} onClick={() => setZoom(stepZoom(zoom(), -1))} />
          <span class="pl-num pl-muted" style={{ "min-width": "52px", "text-align": "center" }}>{Math.round(zoom() * 100)} %</span>
          <IconButton icon="zoom-in" label={t("toolbar.zoomIn")} disabled={!activeDoc()} onClick={() => setZoom(stepZoom(zoom(), 1))} />
          <IconButton icon="maximize" label={t("toolbar.fitWidth")} disabled={!activeDoc()} onClick={() => first() && setZoom(fitWidthZoom(first()!, rotation(), canvas.clientWidth))} />
          <IconButton icon="square" label={t("toolbar.fitPage")} disabled={!activeDoc()} onClick={() => first() && setZoom(fitPageZoom(first()!, rotation(), { width: canvas.clientWidth, height: canvas.clientHeight }))} />
          <IconButton icon="rotate-cw" label={t("toolbar.rotate")} disabled={!activeDoc()} onClick={() => setRotation(rotate(rotation()))} />
          <ToolbarSeparator />
          <SearchBox />
        </>
      ) }),
      registerStatusItem({ id: "viewer.page", slot: "start", hideOnHome: true, text: () => (activeDoc() ? t("status.page", { page: currentPageIndex() + 1, total: activeDoc()!.sizes.length }) : "") }),
    ];
    offs.push(installNavigation());
    setMode("read");
    setActivePanel("viewer.thumbnails");
    const unlisten = await Promise.all([
      listenForChanges(),
      events.onOpenFiles((paths) => paths.forEach((p) => void open(p))),
      getCurrentWebview().onDragDropEvent((e) => {
        if (e.payload.type === "drop") e.payload.paths.filter((p) => /\.pdf$/i.test(p)).forEach((p) => void open(p));
      }),
    ]);
    onCleanup(() => { offs.forEach((off) => off()); unlisten.forEach((off) => off()); });
    for (const p of await api.startupFiles()) await open(p);
  });

  return (
    <AppWindow
      tabs={docs().map((d) => ({ id: String(d.id), title: d.title }))}
      activeId={activeId() === null ? null : String(activeId())}
      onSelectTab={(id) => setActiveId(Number(id))}
      onCloseTab={(id) => void closeDoc(Number(id))}
      onNewTab={() => toast({ tone: "info", text: t("empty.hint") })}
      onReorderTabs={(from, to) => setDocs((all) => { const n = [...all]; n.splice(to, 0, n.splice(from, 1)[0]!); return n; })}
      appMenu={[{ id: "open", label: t("toolbar.open"), icon: "folder-open", shortcut: "Ctrl+O", disabled: true, reason: t("empty.hint") }]}
    >
      <NavigationHost />
      <div ref={canvas} class="viewer-host">
        <Show when={currentScreen()} keyed fallback={<Show when={activeDoc()} keyed fallback={<EmptyState icon="file-text" title={t("empty.title")} hint={t("empty.hint")} />}>
          {(d) => <Viewer doc={d} zoom={zoom()} rotation={rotation()} />}
        </Show>}>{(s) => <Dynamic component={s.component} />}</Show>
      </div>
      <Dialog
        open={passwordFor() !== null} title={t("password.title")} onClose={() => setPasswordFor(null)} onSubmit={() => passwordFor() && void open(passwordFor()!, password())}
        actions={<><Button onClick={() => setPasswordFor(null)}>{t("password.cancel")}</Button><Button type="submit" variant="primary">{t("password.submit")}</Button></>}
      >
        <Field label={t("password.label")} error={wrongPassword() ? t("password.wrong") : undefined}>
          <TextInput type="password" autofocus value={password()} onInput={(e) => setPassword(e.currentTarget.value)} />
        </Field>
      </Dialog>
    </AppWindow>
  );
}
