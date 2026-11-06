/**
 * Autoactualizaciones en la interfaz (W22): pastilla "Actualización lista" en la barra de título (ranura de W19),
 * popover con notas, aviso persistente de obligatorias, avisos de arranque y conexión de Ajustes > Actualizaciones
 * con el servicio real de Rust. Fuera de Tauri (desarrollo, pruebas) no hace nada y Ajustes usa el servicio simulado.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { createEffect, createRoot, createSignal, For, on, Show } from "solid-js";
import { engineText } from "../i18n/engine";
import { activeId, docs } from "../state/documents";
import { setUpdateService, setUpdateSnapshot, settings, updateSnapshot, type UpdateService, type UpdateSnapshot, type UpdateState } from "../settings";
import { isTauri } from "../shell/platform";
import { dismissNotice, registerTitleItem, showNotice } from "../shell/slots";
import { Button, Dialog, Popover, toast } from "../ui";
import { mandatoryNotice, pillKind } from "./logic";
import { ut2 } from "./strings";
import { unsavedDocs, type UnsavedDoc } from "./unsaved";
import "./updates.css";

export { registerUnsavedProvider } from "./unsaved";
export { downloadPercent } from "./logic";

const NOTICE_ID = "update.mandatory";
const [popoverOpen, setPopoverOpen] = createSignal(false);
const [mandatoryHidden, setMandatoryHidden] = createSignal(false);
const [pendingSave, setPendingSave] = createSignal<UnsavedDoc[] | null>(null);

const call = (cmd: string, args?: Record<string, unknown>) => invoke<void>(cmd, args);

async function restartNow(): Promise<void> {
  try { await call("update_restart_now"); } catch (e) { toast({ tone: "danger", text: ut2("restartFailed", { error: engineText(String(e)) }) }); }
}

/** "Reiniciar ahora": si hay documentos con cambios, pide guardarlos antes. */
export async function requestRestart(): Promise<void> {
  setPopoverOpen(false);
  const unsaved = unsavedDocs();
  if (unsaved.length) { setPendingSave(unsaved); return; }
  await restartNow();
}

async function saveAllAndRestart(): Promise<void> {
  const list = pendingSave() ?? [];
  setPendingSave(null);
  for (const d of list) if (!(await d.save())) return;
  await restartNow();
}

function Pill() {
  let anchor: HTMLButtonElement | undefined;
  const kind = () => pillKind(updateSnapshot());
  const s = updateSnapshot;
  return (
    <Show when={kind()}>
      <button ref={anchor} type="button" class="pl-updpill" aria-haspopup="dialog" aria-expanded={popoverOpen()} onClick={() => setPopoverOpen(!popoverOpen())}>
        <span class="pl-updpill__dot" aria-hidden="true" />{kind() === "ready" ? ut2("pill.ready") : ut2("pill.available")}
      </button>
      <Popover anchor={anchor} open={popoverOpen()} onClose={() => setPopoverOpen(false)} placement="bottom-end" label={ut2("pill.ready")} class="pl-updpop" role="dialog">
        <Show when={s()} keyed>{(snap) => (snap.phase === "ready" || snap.phase === "available") ? <PopoverBody snap={snap} /> : null}</Show>
      </Popover>
    </Show>
  );
}

function PopoverBody(props: { snap: UpdateSnapshot & ({ phase: "ready" } | { phase: "available" }) }) {
  const ready = () => props.snap.phase === "ready";
  const elevated = () => props.snap.phase === "ready" && props.snap.needs_elevation;
  return (
    <div class="pl-updpop__body">
      <h2>{ut2("pop.title", { version: props.snap.version })}</h2>
      <p class="pl-muted">{ready() ? ut2("pop.subtitle") : ut2("pop.available")}</p>
      <h3>{ut2("pop.notes")}</h3>
      <ul class="pl-updpop__notes">
        <For each={(props.snap.notes ?? "").split("\n").map((l) => l.replace(/^[-*]\s*/, "").trim()).filter(Boolean)} fallback={<li class="pl-muted">{ut2("pop.noNotes")}</li>}>{(l) => <li>{l}</li>}</For>
      </ul>
      <div class="pl-updpop__actions">
        <Show when={ready()} fallback={<>
          <Button variant="primary" icon="download" onClick={() => { void call("update_download"); setPopoverOpen(false); }}>{ut2("pop.download")}</Button>
          <Button variant="ghost" onClick={() => { void call("update_skip_version"); setPopoverOpen(false); }}>{ut2("pop.skip")}</Button>
        </>}>
          <Button variant="primary" icon={elevated() ? "shield-check" : "refresh"} onClick={() => void requestRestart()}>{elevated() ? ut2("pop.restartElevated") : ut2("pop.restart")}</Button>
          <Button variant="ghost" onClick={() => { setPopoverOpen(false); toast({ tone: "info", text: elevated() ? ut2("pop.laterMachine") : ut2("pop.laterDone") }); }}>{ut2("pop.later")}</Button>
        </Show>
      </div>
    </div>
  );
}

function SavePrompt() {
  return (
    <Dialog open={pendingSave() !== null} title={ut2("unsaved.title")} onClose={() => setPendingSave(null)}
      actions={<><Button variant="ghost" onClick={() => setPendingSave(null)}>{ut2("unsaved.cancel")}</Button><Button variant="primary" onClick={() => void saveAllAndRestart()}>{ut2("unsaved.save")}</Button></>}>
      <p>{ut2("unsaved.text")}</p>
      <ul><For each={pendingSave() ?? []}>{(d) => <li>{d.title}</li>}</For></ul>
    </Dialog>
  );
}

/** Servicio para Ajustes y para la tarjeta de novedades de Inicio, apoyado en el estado real. */
function realService(): UpdateService {
  const snap = () => updateSnapshot();
  const when = (s: UpdateSnapshot): Date => (s.last_check ? new Date(s.last_check) : new Date());
  return {
    currentVersion: () => snap()?.current_version ?? "0.0.0",
    history: () => (snap()?.history ?? []).map(([version, at]) => ({ version, date: at.slice(0, 10) })).reverse(),
    whatsNew: () => { const s = snap(); return s?.whats_new ? { version: s.whats_new, text: s.whats_new_notes ?? "" } : null; },
    check: () => new Promise<UpdateState>((resolve) => {
      let started = false;
      createRoot((dispose) => {
        createEffect(() => {
          const s = snap();
          if (!s) return;
          if (s.busy) { started = true; return; }
          if (!started) return;
          dispose();
          resolve(s.phase === "available" || s.phase === "ready" ? { kind: "available", version: s.version, lastCheck: when(s) } : { kind: "up-to-date", lastCheck: when(s) });
        });
      });
      void call("update_check_now");
    }),
  };
}

/** Monta todo. Devuelve cómo deshacerlo. */
export async function installUpdates(): Promise<() => void> {
  if (!isTauri()) return () => undefined;
  let state: UpdateSnapshot | null = null;
  try { state = await invoke<UpdateSnapshot>("update_status"); } catch { /* servicio no disponible */ }
  setUpdateSnapshot(state);
  const unlisten = await listen<UpdateSnapshot>("update-state-changed", (e) => setUpdateSnapshot(e.payload));
  setUpdateService(realService());
  const offs: (() => void)[] = [
    unlisten,
    registerTitleItem({ id: "updates.pill", order: 50, component: () => <><Pill /><SavePrompt /></> }),
  ];
  offs.push(createRoot((dispose) => {
    // Ajustes (modo, canal, conexión medida) → servicio; la directiva sigue mandando en Rust.
    createEffect(on(() => JSON.stringify(settings().updates), () => { if (updateSnapshot()) void call("update_set_settings", { value: settings().updates }); }, { defer: true }));
    // Aviso persistente de obligatorias sobre el documento activo; se puede cerrar y vuelve en el siguiente arranque.
    createEffect(() => {
      const m = mandatoryNotice(updateSnapshot());
      for (const d of docs()) {
        const id = String(d.id);
        if (!m || mandatoryHidden() || String(activeId()) !== id) { dismissNotice(id, NOTICE_ID); continue; }
        showNotice(id, {
          id: NOTICE_ID, tone: "warn", text: ut2("mandatory", { version: m.version }), dismissible: true, onDismiss: () => setMandatoryHidden(true),
          actions: [m.ready ? { label: ut2("mandatory.action"), onAction: () => void requestRestart() } : { label: ut2("mandatory.download"), onAction: () => void call("update_download") }],
        });
      }
    });
    // Vuelta atrás comunicada una sola vez.
    createEffect(on(() => updateSnapshot()?.rollback, (r) => {
      if (!r) return;
      toast({ tone: "warn", text: ut2("rollback", { failed: r[0], restored: r[1] }) });
      void call("update_dismiss_rollback");
    }));
    return dispose;
  }));
  offs.push(() => setUpdateSnapshot(null));
  return () => offs.forEach((off) => off());
}
