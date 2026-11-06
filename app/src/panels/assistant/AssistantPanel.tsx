import { createResource, createSignal, For, Match, onCleanup, onMount, Show, Switch as SwitchBlock } from "solid-js";
import { goToPage } from "../../state/navigation";
import { Button, Chip, EmptyState, Field, IconButton, Notice, ProgressBar, Tabs, TextInput, toast } from "../../ui";
import { isApiError } from "../../api";
import { locale } from "../../i18n";
import { engineText } from "../../i18n/engine";
import { assistantApi, assistantEvents, type KeyDatum, type ModelInfo, type Status, type Summary } from "./api";
import { addMessage, chatOf, clearChat, formatSize, newJobId, progressFraction, sourcePages, splitCitations } from "./chat";
import { at } from "./strings";
import "./AssistantPanel.css";

interface Job { id: string; label: string; fraction: number | null }

// Resultados que se conservan mientras la app está abierta (no se guardan en disco).
const [summaries, setSummaries] = createSignal<Record<string, Summary>>({});
const [keyData, setKeyData] = createSignal<Record<string, KeyDatum[]>>({});

export function AssistantPanel(props: { docId: string | null }) {
  const [status, { mutate }] = createResource(assistantApi.status);
  const [tab, setTab] = createSignal("ask");
  const [job, setJob] = createSignal<Job | null>(null);
  const [question, setQuestion] = createSignal("");

  onMount(async () => {
    const off = await assistantEvents.onProgress((e) => {
      setJob((j) => (j && j.id === e.job_id ? { ...j, label: e.label, fraction: progressFraction(e) } : j));
    });
    onCleanup(off);
  });

  /** Ejecuta un trabajo con progreso y cancelación; los errores se devuelven como texto, la cancelación en silencio. */
  async function run<T>(label: string, work: (jobId: string) => Promise<T>): Promise<{ ok: true; value: T } | { ok: false; error: string | null }> {
    const id = newJobId();
    setJob({ id, label, fraction: null });
    try {
      return { ok: true, value: await work(id) };
    } catch (e) {
      if (isApiError(e) && e.code === "cancelled") return { ok: false, error: null };
      return { ok: false, error: engineText(isApiError(e) ? e.message : String(e)) };
    } finally {
      setJob(null);
    }
  }
  const cancel = () => { const j = job(); if (j) void assistantApi.cancel(j.id); };

  async function simple(call: () => Promise<Status>) {
    try { mutate(await call()); } catch (e) { toast({ tone: "danger", text: at("error", { message: engineText(isApiError(e) ? e.message : String(e)) }) }); }
  }

  async function download(m: ModelInfo) {
    const r = await run(at("downloading"), (id) => assistantApi.download(m.id, id));
    if (r.ok) mutate(r.value);
    else if (r.error) toast({ tone: "danger", text: at("error", { message: r.error }) });
  }

  const doc = () => (props.docId === null ? null : Number(props.docId));

  async function ask(e: Event) {
    e.preventDefault();
    const q = question().trim(); const d = doc();
    if (!q || d === null || props.docId === null) return;
    const key = props.docId;
    addMessage(key, { role: "user", text: q });
    setQuestion("");
    const r = await run(at("thinking"), (id) => assistantApi.ask(d, q, id));
    if (r.ok) addMessage(key, { role: "assistant", text: r.value.text, citations: r.value.citations });
    else if (r.error) addMessage(key, { role: "error", text: at("error", { message: r.error }) });
  }

  async function summarize() {
    const d = doc(); const key = props.docId;
    if (d === null || key === null) return;
    const r = await run(at("tab.summary"), (id) => assistantApi.summarize(d, id));
    if (r.ok) setSummaries((all) => ({ ...all, [key]: r.value }));
    else if (r.error) toast({ tone: "danger", text: at("error", { message: r.error }) });
  }

  async function findData() {
    const d = doc(); const key = props.docId;
    if (d === null || key === null) return;
    const r = await run(at("tab.data"), (id) => assistantApi.keyData(d, id));
    if (r.ok) setKeyData((all) => ({ ...all, [key]: r.value }));
    else if (r.error) toast({ tone: "danger", text: at("error", { message: r.error }) });
  }

  return (
    <section class="pl-assistant" aria-label={at("title")} data-testid="assistant-panel">
      <Show when={status()} fallback={<p class="pl-muted pl-assistant__pad">…</p>}>
        {(s) => (
          <SwitchBlock>
            <Match when={!s().enabled}>
              <EmptyState icon="message-square" title={at("off.title")} hint={at("off.hint")}>
                <p class="pl-muted">{at("off.needs")}</p>
                <Button variant="primary" onClick={() => void simple(() => assistantApi.setEnabled(true))}>{at("off.enable")}</Button>
              </EmptyState>
            </Match>
            <Match when={!s().model_id || !s().models.some((m) => m.id === s().model_id && m.installed)}>
              <ModelChooser status={s()} job={job()} onDownload={download} onUse={(m) => void simple(() => assistantApi.selectModel(m.id))}
                onCancel={cancel} onDisable={() => void simple(() => assistantApi.setEnabled(false))} />
            </Match>
            <Match when={true}>
              <div class="pl-assistant__head">
                <Chip icon="shield-check" tone="ok">{at("local")}</Chip>
                <IconButton icon="x" label={at("disable")} size="sm" onClick={() => void simple(() => assistantApi.setEnabled(false))} />
              </div>
              <Show when={!s().engine_available}><Notice tone="warn" inline>{at("noEngine")}</Notice></Show>
              <Show when={props.docId !== null} fallback={<EmptyState icon="file-text" title={at("noDoc")} />}>
                <Tabs label={at("tabs")} value={tab()} onChange={setTab} variant="pill" tabs={[
                  { id: "ask", label: at("tab.ask") }, { id: "summary", label: at("tab.summary") }, { id: "data", label: at("tab.data") },
                ]} />
                <div class="pl-assistant__body">
                  <SwitchBlock>
                    <Match when={tab() === "ask"}>
                      <Conversation docId={props.docId!} />
                    </Match>
                    <Match when={tab() === "summary"}>
                      <SummaryView text={summaries()[props.docId!]} busy={job() !== null} onRun={() => void summarize()} />
                    </Match>
                    <Match when={tab() === "data"}>
                      <KeyDataView items={keyData()[props.docId!]} busy={job() !== null} onRun={() => void findData()} />
                    </Match>
                  </SwitchBlock>
                </div>
                <Show when={job()}>{(j) => <div class="pl-assistant__job"><ProgressBar label={j().label} value={j().fraction} onCancel={cancel} /></div>}</Show>
                <Show when={tab() === "ask"}>
                  <form class="pl-assistant__form" onSubmit={(e) => void ask(e)}>
                    <Field label={at("ask.label")} class="pl-assistant__field">
                      <TextInput value={question()} placeholder={at("ask.placeholder")} autocomplete="off" disabled={job() !== null}
                        onInput={(e) => setQuestion(e.currentTarget.value)} />
                    </Field>
                    <Button type="submit" variant="primary" icon="arrow-right" disabled={!question().trim() || job() !== null}>{at("ask.send")}</Button>
                  </form>
                </Show>
              </Show>
            </Match>
          </SwitchBlock>
        )}
      </Show>
    </section>
  );
}

function ModelChooser(props: {
  status: Status; job: Job | null; onDownload: (m: ModelInfo) => void; onUse: (m: ModelInfo) => void; onCancel: () => void; onDisable: () => void;
}) {
  return (
    <div class="pl-assistant__choose">
      <h2>{at("choose.title")}</h2>
      <p class="pl-muted">{at("choose.hint")}</p>
      <Show when={!props.status.engine_available}><Notice tone="warn" inline>{at("noEngine")}</Notice></Show>
      <ul class="pl-assistant__models">
        <For each={props.status.models}>{(m) => (
          <li class="pl-assistant__model">
            <div class="pl-assistant__model-head">
              <strong>{m.name}</strong>
              <Show when={m.recommended}><Chip tone="accent">{at("model.recommended")}</Chip></Show>
            </div>
            <p class="pl-muted">{m.notes}</p>
            <p class="pl-assistant__meta">
              <span>{at("model.license", { license: m.license })}</span>
              <span>{at("model.size", { size: formatSize(m.requirements.download_mb, locale()) })}</span>
              <span>{at("model.ram", { ram: m.requirements.ram_gb })}</span>
            </p>
            <Show when={m.installed} fallback={<Button variant={m.recommended ? "primary" : "normal"} icon="download" disabled={props.job !== null} onClick={() => props.onDownload(m)}>{at("model.download")}</Button>}>
              <Button variant="primary" onClick={() => props.onUse(m)}>{at("model.use")}</Button>
            </Show>
          </li>
        )}</For>
      </ul>
      <Show when={props.job}>{(j) => <ProgressBar label={j().label} value={j().fraction} onCancel={props.onCancel} />}</Show>
      <Button variant="ghost" onClick={props.onDisable}>{at("disable")}</Button>
    </div>
  );
}

function PageChip(props: { page: number }) {
  return (
    <Chip tone="accent" title={at("goToPage", { n: props.page + 1 })} onClick={() => goToPage(props.page)}>{at("page", { n: props.page + 1 })}</Chip>
  );
}

function Conversation(props: { docId: string }) {
  let log!: HTMLDivElement;
  const messages = () => chatOf(props.docId);
  // Mantiene visible lo último que se escribió.
  const scroll = () => queueMicrotask(() => { if (log) log.scrollTop = log.scrollHeight; });
  return (
    <div class="pl-assistant__chat">
      <Show when={messages().length} fallback={<p class="pl-muted pl-assistant__pad">{at("ask.empty")}</p>}>
        <div class="pl-assistant__log" role="log" aria-live="polite" ref={log} data-testid="assistant-log">
          <For each={messages()}>{(m) => {
            scroll();
            return (
              <div class={`pl-assistant__msg pl-assistant__msg--${m.role}`}>
                <SwitchBlock>
                  <Match when={m.role === "assistant" && m}>{(a) => (
                    <>
                      <p class="pl-assistant__text">
                        <For each={splitCitations(engineText(a().text))}>{(seg) => seg.kind === "text" ? <>{seg.text}</> : <PageChip page={seg.page} />}</For>
                      </p>
                      <Show when={a().citations.length && !a().citations.some((c) => c.cited_by_model)}>
                        <div class="pl-assistant__sources">
                          <span class="pl-muted">{at("ask.sourceHint")}</span>
                          <For each={sourcePages(a().citations)}>{(p) => <PageChip page={p} />}</For>
                        </div>
                      </Show>
                    </>
                  )}</Match>
                  <Match when={m.role !== "assistant"}><p class="pl-assistant__text">{m.text}</p></Match>
                </SwitchBlock>
              </div>
            );
          }}</For>
        </div>
        <Button variant="ghost" icon="trash" onClick={() => clearChat(props.docId)}>{at("ask.clear")}</Button>
      </Show>
    </div>
  );
}

function SummaryView(props: { text: Summary | undefined; busy: boolean; onRun: () => void }) {
  const [copied, setCopied] = createSignal(false);
  const copy = async (t: string) => {
    try { await navigator.clipboard.writeText(t); setCopied(true); setTimeout(() => setCopied(false), 1500); } catch { /* sin portapapeles */ }
  };
  return (
    <div class="pl-assistant__view">
      <p class="pl-muted">{at("summary.hint")}</p>
      <Button variant="primary" disabled={props.busy} onClick={props.onRun}>{at("summary.run")}</Button>
      <Show when={props.text}>{(s) => (
        <article class="pl-assistant__result">
          <h3>{at("summary.pages", { n: s().pages_read })}</h3>
          <p class="pl-assistant__text">
            <For each={splitCitations(s().text)}>{(seg) => seg.kind === "text" ? <>{seg.text}</> : <PageChip page={seg.page} />}</For>
          </p>
          <Button variant="ghost" icon="copy" onClick={() => void copy(s().text)}>{copied() ? at("copied") : at("copy")}</Button>
        </article>
      )}</Show>
    </div>
  );
}

function KeyDataView(props: { items: KeyDatum[] | undefined; busy: boolean; onRun: () => void }) {
  const of = (kind: KeyDatum["kind"]) => (props.items ?? []).filter((i) => i.kind === kind);
  return (
    <div class="pl-assistant__view">
      <p class="pl-muted">{at("data.hint")}</p>
      <Button variant="primary" disabled={props.busy} onClick={props.onRun}>{at("data.run")}</Button>
      <Show when={props.items}>
        <Show when={props.items!.length} fallback={<p class="pl-muted">{at("data.none")}</p>}>
          <For each={(["date", "amount"] as const).filter((k) => of(k).length)}>{(kind) => (
            <section class="pl-assistant__result">
              <h3>{at(kind === "date" ? "data.date" : "data.amount")}</h3>
              <ul class="pl-assistant__data">
                <For each={of(kind)}>{(i) => <li><span>{i.value}</span><PageChip page={i.page} /></li>}</For>
              </ul>
            </section>
          )}</For>
        </Show>
      </Show>
    </div>
  );
}
