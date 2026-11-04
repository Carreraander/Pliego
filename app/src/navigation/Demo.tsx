import { onCleanup, onMount } from "solid-js";
import { RadioGroup, Radio, TaskPanel, EmptyState } from "../ui";
import { AppWindow, registerMode, registerStatusItem, setMode } from "../shell";
import { setActiveId, setDocs } from "../state/documents";
import { registerTool, setDocFacts } from "../tools";
import { installNavigation, NavigationHost } from "./index";
import { installHome, memoryBackend, setRecentsBackend, openHome, openSettings, refreshRecents } from "../home";
import { onFinished, onProgress, trackJob } from "../jobs/state";
import { installJobs } from "../jobs";
import { currentScreen } from "./screen";
import { Dynamic } from "solid-js/web";
import { Show } from "solid-js";
import { initSettings } from "../settings";

/** Panel de tarea de ejemplo: lo que añadiría un flujo (aquí «Comprimir» de W08) en `app/src/tools/optimize/`. */
function CompressPanel(p: { onClose: () => void }) {
  return (
    <TaskPanel title="Comprimir" onClose={p.onClose} estimate="Ahora 12,4 MB → estimado ~3,1 MB" primary={{ label: "Guardar copia", onClick: () => p.onClose() }}>
      <RadioGroup legend="Calidad" value="balanced"><Radio value="max" label="Máxima calidad" /><Radio value="balanced" label="Equilibrado" /><Radio value="min" label="Mínimo tamaño" /></RadioGroup>
    </TaskPanel>
  );
}

/** Navegación con un documento falso, para probar paletas, modos y atajos sin Tauri (solo desarrollo): `/nav-demo`. */
export function NavigationDemo() {
  const onScreen = () => currentScreen() !== null && currentScreen() !== undefined;
  const day = 86_400;
  const nowSec = Math.floor(Date.now() / 1000);
  setRecentsBackend(memoryBackend([
    { path: "C:\\Docs\\contrato.pdf", name: "contrato.pdf", folder: "C:\\Docs", opened_at: nowSec - 600, pinned: true, exists: true, size: 120_000 },
    { path: "C:\\Docs\\factura-octubre.pdf", name: "factura-octubre.pdf", folder: "C:\\Docs", opened_at: nowSec - day, pinned: false, exists: true, size: 30_000 },
    { path: "D:\\Antiguo\\informe-anual.pdf", name: "informe-anual.pdf", folder: "D:\\Antiguo", opened_at: nowSec - 12 * day, pinned: false, exists: false, size: null },
  ]));
  onMount(() => {
    void initSettings();
    void refreshRecents();
    // Ganchos para las pruebas: simulan los eventos de Tauri de los trabajos.
    (window as unknown as Record<string, unknown>).__demo = {
      progress: (id: number, done: number, total: number, label: string) => onProgress({ job_id: id, done, total, label }),
      finished: (id: number, ok: boolean) => onFinished({ job_id: id, result: ok ? { outputs: ["C:\\Docs\\contrato (comprimido).pdf"], warnings: [], summary: "12,4 MB → 3,1 MB" } : null, error: ok ? null : { code: "other", message: "No se pudo reconocer el texto: archivo dañado", cancelled: false } }),
      track: (id: number, title: string) => trackJob(id, { title }),
      home: openHome, settings: openSettings,
    };
    setDocs([
      { id: 1, path: "C:\\Docs\\contrato.pdf", title: "contrato.pdf", sizes: Array.from({ length: 12 }, () => ({ width: 595, height: 842 })), revision: 0 },
      { id: 2, path: "C:\\Docs\\factura-octubre.pdf", title: "factura-octubre.pdf", sizes: [{ width: 595, height: 842 }], revision: 0 },
    ]);
    setActiveId(1);
    setDocFacts({ document: true });
    const offs = [
      registerMode({ id: "read", title: "Leer", icon: "mouse-pointer", order: 10, toolbar: () => <span class="pl-muted">Barra de Leer</span> }),
      registerStatusItem({ id: "demo.page", slot: "start", hideOnHome: true, text: () => "Pág. 1 de 12" }),
      installNavigation(),
      // Una herramienta nueva = un manifiesto + una línea de registro (sustituye a la fantasma con el mismo id).
      // Va después de installNavigation: aquí el panel de ejemplo sustituye al real de W24 para las pruebas visuales.
      registerTool({ id: "optimize.compress", group: "compare-optimize", kind: "task", title: "tools.optimize.compress.title", description: "tools.optimize.compress.description",
        icon: "file-down", keywords: ["reducir", "tamaño"], when: { document: true }, pinnable: true, load: async () => ({ default: CompressPanel }) }),
      installHome(() => undefined),
    ];
    void installJobs(() => undefined).then((off) => offs.push(off));
    setMode("read");
    onCleanup(() => offs.forEach((off) => off()));
  });
  return (
    <AppWindow tabs={onScreen() ? [{ id: "screen", title: currentScreen()!.title?.() ?? currentScreen()!.id, kind: "screen" as const }, { id: "1", title: "contrato.pdf" }] : [{ id: "1", title: "contrato.pdf" }, { id: "2", title: "factura-octubre.pdf" }]} activeId={onScreen() ? "screen" : "1"} onSelectTab={() => {}} onCloseTab={() => {}} onNewTab={() => {}} onReorderTabs={() => {}} appMenu={[]}>
      <NavigationHost />
      <Show when={currentScreen()} keyed fallback={<EmptyState icon="file-text" title="Documento de ejemplo" hint="Prueba Ctrl+K, Ctrl+Mayús+A, Alt+1…9 y F1." />}>{(s) => <Dynamic component={s.component} />}</Show>
    </AppWindow>
  );
}
