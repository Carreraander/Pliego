import { createMemo, createResource, createSignal, For, Show, type JSX } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { Button, Icon, Notice, Select, Switch, TextInput } from "../ui";
import { bestScore } from "../navigation/fuzzy";
import { openShortcutSheet } from "../navigation/ShortcutSheet";
import { isTauri } from "../shell/platform";
import { accessibilityRows, extraSections, generalRows, UpdateModeToggle, viewerRows, type SettingRow, type SettingsSection } from "./sections";
import { resetSettings, settings, updateSettings } from "./store";
import { st } from "./strings";
import { checkForUpdates, updateService, updateSnapshot, updateState } from "./updates";
import { downloadPercent, requestRestart } from "../updates";
import "./settings.css";

interface About { version: string; license: string; source_url: string; components: { name: string; version: string; license: string }[] }

async function loadAbout(): Promise<About> {
  if (isTauri()) return invoke<About>("about_info");
  return { version: updateService().currentVersion(), license: "AGPL-3.0-or-later", source_url: "https://github.com/Carreraander/Pliego", components: [] };
}

function Row(props: { row: SettingRow }) {
  return (
    <div class="pl-row">
      <div><span class="pl-row__label">{props.row.label()}</span><Show when={props.row.hint}><span class="pl-row__hint">{props.row.hint!()}</span></Show></div>
      <div>{props.row.control()}</div>
    </div>
  );
}
const Rows = (props: { rows: SettingRow[] }) => <div class="pl-card"><For each={props.rows}>{(r) => <Row row={r} />}</For></div>;

function formatWhen(d: Date | null): string {
  return d ? new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(d) : st("updates.never");
}

function UpdatesSection() {
  const state = updateState;
  const real = updateSnapshot;
  const managed = () => real()?.managed === true;
  const text = () => {
    const s = state();
    const r = real();
    if (r?.phase === "not-configured") return st("updates.notConfigured");
    if (r?.phase === "disabled" || (!r && settings().updates.mode === "off") || r?.mode === "disabled") return st("updates.off");
    if (r?.phase === "downloading") return st("updates.downloading", { version: "", pct: downloadPercent(r) ?? 0 });
    if (r?.phase === "ready") return st("updates.ready", { version: r.version });
    if (r?.phase === "failed") return r.message;
    if (r?.phase === "available") return st("updates.available", { version: r.version });
    if (s.kind === "checking" || r?.busy) return st("updates.checking");
    if (s.kind === "available") return st("updates.available", { version: s.version });
    return st("updates.upToDate");
  };
  const last = () => { const r = real(); if (r) return r.last_check ? new Date(r.last_check) : null; const s = state(); return s.kind === "up-to-date" || s.kind === "available" ? s.lastCheck : s.kind === "idle" ? s.lastCheck : null; };
  const off = () => (real() ? real()!.mode === "disabled" || real()!.phase === "not-configured" : settings().updates.mode === "off");
  return (
    <>
      <Show when={managed()}><Notice tone="info">{st("updates.managed")}</Notice></Show>
      <div class="pl-card pl-update" role="status">
        <span class="pl-update__ring"><Icon name="refresh" size={20} /></span>
        <div class="pl-update__text"><b>{text()}</b><span class="pl-row__hint">{updateService().currentVersion()} · {st("updates.lastCheck", { when: formatWhen(last()) })}</span></div>
        <Show when={real()?.phase === "available" && (real() as { can_download?: boolean }).can_download}><Button onClick={() => void invoke("update_download")} icon="download">{st("updates.download")}</Button></Show>
        <Show when={real()?.phase === "ready"}><Button variant="primary" onClick={() => void requestRestart()} icon="refresh">{st("updates.restart")}</Button></Show>
        <Button onClick={() => void checkForUpdates()} disabled={state().kind === "checking" || real()?.busy || off()} icon="refresh">{st("updates.checkNow")}</Button>
      </div>
      <div class="pl-card">
        <div class="pl-row"><div><span class="pl-row__label">{st("row.updates.mode")}</span><span class="pl-row__hint">{st("row.updates.mode.hint")}</span></div>
          <Select aria-label={st("row.updates.mode")} disabled={managed()} value={real() ? ({ automatic: "auto", "notify-only": "notify", disabled: "off" } as const)[real()!.mode] : settings().updates.mode} onChange={(e) => updateSettings("updates", { mode: e.currentTarget.value as "auto" })}
            options={[{ value: "auto", label: st("updates.mode.auto") }, { value: "notify", label: st("updates.mode.notify") }, { value: "off", label: st("updates.mode.off") }]} /></div>
        <div class="pl-row"><div><span class="pl-row__label">{st("row.updates.channel")}</span><span class="pl-row__hint">{st("row.updates.channel.hint")}</span></div><UpdateModeToggle disabled={managed()} /></div>
        <div class="pl-row"><div><span class="pl-row__label">{st("row.updates.metered")}</span><span class="pl-row__hint">{st("row.updates.metered.hint")}</span></div>
          <Switch label="" aria-label={st("row.updates.metered")} disabled={managed()} checked={settings().updates.metered} onChange={(e) => updateSettings("updates", { metered: e.currentTarget.checked })} /></div>
      </div>
      <div class="pl-card"><div class="pl-row"><div><span class="pl-row__label">{st("updates.history")}</span>
        <span class="pl-row__hint pl-num">{updateService().history().map((h) => `${h.version} · ${h.date}`).join("  ·  ")}</span></div></div>
        <Show when={real()?.server}><div class="pl-row"><div><span class="pl-row__label">{st("updates.server")}</span><span class="pl-row__hint pl-num">{real()!.server}</span></div></div></Show></div>
      <Show when={!real()}><Notice tone="info">{st("updates.simulated")}</Notice></Show>
      <p class="pl-row__hint">{st("updates.privacy")}</p>
    </>
  );
}

function AdvancedSection() {
  const [logsError, setLogsError] = createSignal("");
  return (
    <>
      <div class="pl-card">
        <div class="pl-row"><div><span class="pl-row__label">{st("advanced.shortcuts")}</span><span class="pl-row__hint">{st("advanced.shortcuts.hint")}</span></div>
          <Button icon="keyboard" onClick={openShortcutSheet}>{st("advanced.shortcuts.open")}</Button></div>
        <div class="pl-row"><div><span class="pl-row__label">{st("advanced.reset")}</span><span class="pl-row__hint">{st("advanced.reset.hint")}</span></div>
          <Button variant="danger" onClick={resetSettings}>{st("advanced.reset.button")}</Button></div>
      </div>
      <Show when={isTauri()}>
        <div class="pl-card"><div class="pl-row"><div><span class="pl-row__label">{st("advanced.logs")}</span><span class="pl-row__hint">{st("advanced.logs.hint")}</span></div>
          <Button icon="folder-open" onClick={() => void invoke("open_logs_folder").catch((e) => setLogsError(String(e)))}>{st("advanced.logs.open")}</Button></div></div>
        <Show when={logsError()}><Notice tone="warn">{logsError()}</Notice></Show>
      </Show>
      <p class="pl-row__hint">{st("advanced.dataDir")}</p>
    </>
  );
}

function AboutSection() {
  const [about] = createResource(loadAbout);
  return (
    <div class="pl-about">
      <div class="pl-card">
        <dl>
          <dt>{st("about.version")}</dt><dd>{about()?.version ?? "…"}</dd>
          <dt>{st("about.license")}</dt><dd>{about()?.license ?? "AGPL-3.0-or-later"}</dd>
          <dt>{st("about.source")}</dt><dd><Show when={about()}>{(a) => <a class="pl-link" href={a().source_url} target="_blank" rel="noreferrer">{a().source_url}</a>}</Show></dd>
        </dl>
      </div>
      <p>{st("about.tagline")}</p>
      <p class="pl-row__hint">{st("about.licenseText")} {st("about.sourceNote")}</p>
      <Show when={about()?.components.length}>
        <h3>{st("about.components")}</h3>
        <div class="pl-card"><For each={about()!.components}>{(c) => (
          <div class="pl-row"><span>{c.name} <span class="pl-num pl-muted">{c.version}</span></span><span class="pl-muted">{c.license}</span></div>
        )}</For></div>
      </Show>
    </div>
  );
}

const sections = (): SettingsSection[] => [
  { id: "general", title: () => st("section.general"), icon: "wrench", order: 10, rows: generalRows },
  { id: "viewer", title: () => st("section.viewer"), icon: "file-text", order: 20, rows: viewerRows },
  { id: "updates", title: () => st("section.updates"), icon: "refresh", order: 60, component: UpdatesSection, keywords: ["actualizar", "versión", "canal", "beta", "update"] },
  { id: "accessibility", title: () => st("section.accessibility"), icon: "accessibility", order: 80, rows: accessibilityRows },
  { id: "advanced", title: () => st("section.advanced"), icon: "keyboard", order: 90, component: AdvancedSection, keywords: ["atajos", "teclado", "restablecer", "shortcuts", "reset"] },
  { id: "about", title: () => st("section.about"), icon: "info", order: 100, component: AboutSection, keywords: ["licencia", "agpl", "código fuente", "pdfium", "versión", "license"] },
  ...extraSections(),
].sort((a, b) => (a.order ?? 50) - (b.order ?? 50));

const [current, setCurrent] = createSignal("general");
/** Sección que se muestra al abrir Ajustes (la paleta de órdenes salta a una concreta). */
export const showSettingsSection = (id: string): void => { setCurrent(id); };
export const sectionIds = (): string[] => sections().map((s) => s.id);

/** Ajustes como pestaña: secciones a la izquierda, buscador arriba, cambios con efecto inmediato (§3.5). */
export function Settings() {
  const [q, setQ] = createSignal("");
  const results = createMemo(() => {
    const query = q().trim();
    if (!query) return null;
    return sections().flatMap((s) => {
      const hits: JSX.Element[] = [];
      const titleHit = bestScore(query, [s.title(), ...(s.keywords ?? [])]) !== null;
      const rows = (s.rows ?? []).filter((r) => bestScore(query, [r.label(), ...(r.keywords ?? [])], r.hint ? [r.hint()] : []) !== null);
      if (rows.length) hits.push(<><h3>{s.title()}</h3><Rows rows={rows} /></>);
      else if (titleHit && s.component) hits.push(<><h3>{s.title()}</h3><s.component /></>);
      else if (titleHit && s.rows) hits.push(<><h3>{s.title()}</h3><Rows rows={s.rows} /></>);
      return hits;
    });
  });
  const active = () => sections().find((s) => s.id === current()) ?? sections()[0]!;
  return (
    <div class="pl-settings" data-testid="settings">
      <nav class="pl-settings__nav" aria-label={st("settings.nav")}>
        <TextInput type="search" aria-label={st("settings.search")} placeholder={st("settings.search")} value={q()} onInput={(e) => setQ(e.currentTarget.value)} />
        <For each={sections()}>{(s) => (
          <button type="button" class="pl-settings__navitem" aria-current={!q() && active().id === s.id ? "true" : undefined} onClick={() => { setQ(""); setCurrent(s.id); }}>
            <Icon name={s.icon} size={16} />{s.title()}
          </button>
        )}</For>
      </nav>
      <main class="pl-settings__main" aria-live="polite">
        <Show when={results()} fallback={<>
          <h2>{active().title()}</h2>
          <Show when={active().rows}><Rows rows={active().rows!} /></Show>
          <Show when={active().component} keyed>{(C) => <C />}</Show>
        </>}>
          {(list) => <>
            <h2>{st("settings.title")}</h2>
            <For each={list()} fallback={<p class="pl-settings__empty">{st("settings.none", { q: q() })}</p>}>{(h) => h}</For>
          </>}
        </Show>
      </main>
    </div>
  );
}

/** Índice de ajustes para la paleta de órdenes: una entrada por sección y por fila. */
export function settingsIndex(): { section: string; sectionTitle: string; label: string; texts: string[]; exact: string[] }[] {
  return sections().flatMap((s) => [
    { section: s.id, sectionTitle: s.title(), label: s.title(), texts: [s.title(), ...(s.keywords ?? [])], exact: [] },
    ...(s.rows ?? []).map((r) => ({ section: s.id, sectionTitle: s.title(), label: r.label(), texts: [r.label(), ...(r.keywords ?? [])], exact: r.hint ? [r.hint()] : [] })),
  ]);
}
