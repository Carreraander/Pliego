import { createResource, createSignal, For, Show } from "solid-js";
import { api } from "../api";
import { activateTool } from "../navigation/activate";
import { openToolsPalette } from "../navigation/ToolsPalette";
import { Button, Chip, ContextMenu, contextPoint, Icon, IconButton, Logo, MenuButton, type MenuEntry, type Point } from "../ui";
import { getTool } from "../tools";
import { DEFAULT_QUICK_ACTIONS, settings, updateSettings } from "../settings/store";
import { updateService } from "../settings/updates";
import { clearRecoverable, recoverable } from "./recovery";
import { pinRecent, recents, removeRecent, revealRecent, thumbnailFor, type Recent } from "./recents";
import { formatOpened, greeting } from "./format";
import { ht } from "./strings";
import "./home.css";

/** Acciones rápidas disponibles: `tool` es el id de la herramienta del registro (sin `tool`, abre el selector de archivos). */
export const QUICK_ACTIONS: { id: string; icon: string; tool?: string }[] = [
  { id: "open", icon: "folder-open" }, { id: "merge", icon: "combine", tool: "pages.merge" }, { id: "ocr", icon: "ocr", tool: "ocr.recognize" },
  { id: "sign", icon: "sign-digital", tool: "sign.digital" }, { id: "compress", icon: "file-down", tool: "optimize.compress" }, { id: "convert", icon: "file-output", tool: "convert.export" },
  { id: "split", icon: "scissors", tool: "pages.split" }, { id: "protect", icon: "lock", tool: "protect.protect" }, { id: "compare", icon: "columns-2", tool: "compare.compare" },
];

function Thumb(props: { path: string; exists: boolean }) {
  const [url] = createResource(() => (props.exists ? props.path : null), (p) => thumbnailFor(p));
  return (
    <span class="pl-recent__thumb" aria-hidden="true">
      <Show when={url()} fallback={<Icon name="file-text" size={18} />}>{(u) => <img src={u()} alt="" draggable={false} />}</Show>
    </span>
  );
}

function RecentRow(props: { item: Recent; onOpen: (path: string) => void; onMenu: (p: Point, r: Recent) => void }) {
  const r = () => props.item;
  const activate = () => (r().exists ? props.onOpen(r().path) : void find());
  const find = async () => { const picked = await api.pickPdfFiles(); if (picked[0]) { await removeRecent(r().path); props.onOpen(picked[0]); } };
  return (
    <li class="pl-recent" classList={{ "pl-recent--missing": !r().exists }} onContextMenu={(e) => props.onMenu(contextPoint(e), r())}>
      <button type="button" class="pl-recent__main" onClick={activate}>
        <Thumb path={r().path} exists={r().exists} />
        <span class="pl-recent__name"><b>{r().name}</b></span>
        <span class="pl-recent__folder pl-muted">{r().folder}</span>
        <span class="pl-recent__date pl-muted pl-num">{formatOpened(r().opened_at)}</span>
        <span class="pl-recent__state"><Show when={!r().exists}><Chip tone="warn">{ht("recent.missing")}</Chip></Show></span>
      </button>
      <span>{!r().exists && <Button variant="ghost" onClick={() => void find()}>{ht("recent.find")}</Button>}</span>
      <IconButton icon="star" label={`${r().pinned ? ht("recent.unpin") : ht("recent.pin")}: ${r().name}`} pressed={r().pinned} onClick={() => void pinRecent(r().path, !r().pinned)} />
    </li>
  );
}

/** Pantalla de Inicio (§3.1): saludo, zona de soltar, acciones rápidas, recientes y pie. */
export function Home(props: { onOpen: (path: string) => void }) {
  const [menu, setMenu] = createSignal<{ point: Point; item: Recent } | null>(null);
  const [newsClosed, setNewsClosed] = createSignal(false);
  const pick = async () => { for (const p of await api.pickPdfFiles(ht("home.pick"))) props.onOpen(p); };
  const run = (a: (typeof QUICK_ACTIONS)[number]) => { if (!a.tool) { void pick(); return; } const tool = getTool(a.tool); if (tool) void activateTool(tool); };
  const chosen = () => settings().home.quickActions.map((id) => QUICK_ACTIONS.find((a) => a.id === id)).filter((a): a is (typeof QUICK_ACTIONS)[number] => !!a);
  const toggleAction = (id: string) => {
    const list = settings().home.quickActions;
    const next = list.includes(id) ? list.filter((x) => x !== id) : [...list, id];
    if (next.length) updateSettings("home", { quickActions: next });
  };
  const customizeItems = (): MenuEntry[] => [
    ...QUICK_ACTIONS.map((a): MenuEntry => ({ id: a.id, label: ht(`qa.${a.id}`), icon: a.icon, checked: settings().home.quickActions.includes(a.id), onSelect: () => toggleAction(a.id) })),
    { type: "separator" },
    { id: "reset", label: ht("home.customize.reset"), icon: "refresh", onSelect: () => updateSettings("home", { quickActions: DEFAULT_QUICK_ACTIONS }) },
  ];
  const news = () => {
    const n = updateService().whatsNew();
    return n && n.version !== settings().seenVersion && !newsClosed() ? n : null;
  };
  const closeNews = () => { const n = news(); if (n) updateSettings("seenVersion", n.version); setNewsClosed(true); };
  const rowMenu = (r: Recent): MenuEntry[] => [
    { id: "open", label: ht("recent.open", { name: r.name }), icon: "folder-open", disabled: !r.exists, onSelect: () => props.onOpen(r.path) },
    { id: "pin", label: r.pinned ? ht("recent.unpin") : ht("recent.pin"), icon: "star", onSelect: () => void pinRecent(r.path, !r.pinned) },
    { id: "reveal", label: ht("recent.reveal"), icon: "folder-open", disabled: !r.exists, onSelect: () => void revealRecent(r.path) },
    { type: "separator" },
    { id: "remove", label: ht("recent.remove"), icon: "trash", danger: true, onSelect: () => void removeRecent(r.path) },
  ];

  return (
    <div class="pl-home" data-testid="home">
      <Show when={recoverable()} keyed>{(rec) => (
        <div class="pl-home__banner pl-home__banner--warn" role="alert">
          <Icon name="alert-triangle" size={18} />
          <span>{rec.docs.length === 1 ? ht("recover.one") : ht("recover.text", { n: rec.docs.length })}</span>
          <Button variant="primary" onClick={() => { void rec.handlers.restore(rec.docs); clearRecoverable(); }}>{ht("recover.restore")}</Button>
          <Button onClick={() => { void rec.handlers.discard(rec.docs); clearRecoverable(); }}>{ht("recover.discard")}</Button>
        </div>
      )}</Show>
      <Show when={news()} keyed>{(n) => (
        <div class="pl-home__banner" role="status">
          <Icon name="sparkles" size={18} />
          <div><b>{ht("news.title", { version: n.version })}</b><div class="pl-muted">{n.text}</div></div>
          <IconButton icon="x" label={ht("news.close")} onClick={closeNews} />
        </div>
      )}</Show>
      <Show when={settings().general.greeting}><h2><Logo size={28} /> {ht(greeting(new Date().getHours()))}</h2></Show>
      <button type="button" class="pl-home__drop" onClick={() => void pick()}>
        <Icon name="download" size={24} />
        <span><b>{ht("home.drop")}</b><span class="pl-muted">{ht("home.dropHint")}</span></span>
      </button>
      <section aria-label={ht("home.quick")}>
        <div class="pl-home__head"><h3>{ht("home.quick")}</h3>
          <MenuButton label={ht("home.customize.label")} items={customizeItems()}>
            {(m) => <Button ref={m.ref} variant="ghost" icon="settings" aria-haspopup="menu" aria-expanded={m.open} onClick={m.toggle}>{ht("home.customize")}</Button>}
          </MenuButton>
        </div>
        <ul class="pl-home__qa">
          <For each={chosen()}>{(a) => (
            <li><button type="button" class="pl-qa" onClick={() => run(a)}>
              <span class="pl-qa__icon"><Icon name={a.icon} size={20} /></span>
              <span><b>{ht(`qa.${a.id}`)}</b><small>{ht(`qa.${a.id}.d`)}</small></span>
            </button></li>
          )}</For>
        </ul>
      </section>
      <section aria-label={ht("home.recents")}>
        <div class="pl-home__head"><h3>{ht("home.recents")}</h3><button type="button" class="pl-link" onClick={() => openToolsPalette()}>{ht("home.allTools")}</button></div>
        <Show when={recents().length} fallback={<p class="pl-muted">{ht("home.recents.none")}</p>}>
          <ul class="pl-home__recents"><For each={recents()}>{(r) => <RecentRow item={r} onOpen={props.onOpen} onMenu={(point, item) => setMenu({ point, item })} />}</For></ul>
        </Show>
      </section>
      <p class="pl-home__footer pl-muted">{ht("home.footer", { version: updateService().currentVersion() })}</p>
      <ContextMenu point={menu()?.point ?? null} onClose={() => setMenu(null)} label={ht("recent.actions", { name: menu()?.item.name ?? "" })} items={menu() ? rowMenu(menu()!.item) : []} />
    </div>
  );
}
