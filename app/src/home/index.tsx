import { locale } from "../i18n";
import { bestScore, closeScreen, openScreen, registerCommandProvider, registerShortcut } from "../navigation";
import { settingsIndex, Settings, showSettingsSection } from "../settings/Settings";
import { Home } from "./Home";
import { addRecent, recents, refreshRecents } from "./recents";
import { ht } from "./strings";

export { Home } from "./Home";
export { addRecent, refreshRecents, setRecentsBackend, memoryBackend, type Recent, type RecentsBackend } from "./recents";
export { setRecoverable, type Recoverable, type RecoveryHandlers } from "./recovery";

let openPdf: (path: string) => void = () => undefined;

export function openHome(): void { openScreen({ id: "home", component: () => <Home onOpen={(p) => { closeScreen(); openPdf(p); }} /> }); }
export function openSettings(section?: string): void {
  if (section) showSettingsSection(section);
  openScreen({ id: "settings", component: Settings });
}

/** Anota la apertura de un documento en los recientes. */
export const recordOpened = (path: string): void => { void addRecent(path); };

/** Monta Inicio y Ajustes en la navegación: pantallas, `Ctrl+,`, recientes y ajustes en la paleta de órdenes. */
export function installHome(open: (path: string) => void): () => void {
  openPdf = open;
  void refreshRecents();
  const offs = [
    registerShortcut({ id: "file.settings", section: "file", keys: "Ctrl+,", title: () => (locale() === "es" ? "Ajustes" : "Settings"), run: () => openSettings() }),
    registerCommandProvider({
      id: "home.recents", order: 25, group: "nav.group.recents",
      query: (q) => recents().filter((r) => !q.trim() || bestScore(q, [r.name], [r.folder]) !== null).map((r) => ({
        id: `recent:${r.path}`, title: r.name, subtitle: r.folder, icon: "file-text", disabledReason: r.exists ? undefined : ht("recent.missing"), run: () => open(r.path),
      })),
    }),
    registerCommandProvider({
      id: "home.settings", order: 40, group: "nav.group.settings",
      query: (q) => (q.trim() ? settingsIndex().filter((i) => bestScore(q, i.texts, i.exact) !== null) : []).map((i) => ({
        id: `settings:${i.section}:${i.label}`, title: i.label === i.sectionTitle ? i.sectionTitle : `${i.sectionTitle} › ${i.label}`, icon: "settings", run: () => openSettings(i.section),
      })),
    }),
  ];
  return () => offs.forEach((off) => off());
}
