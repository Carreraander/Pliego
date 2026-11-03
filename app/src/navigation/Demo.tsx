import { onCleanup, onMount } from "solid-js";
import { RadioGroup, Radio, TaskPanel, EmptyState } from "../ui";
import { AppWindow, registerMode, registerStatusItem, setMode } from "../shell";
import { setActiveId, setDocs } from "../state/documents";
import { registerTool, setDocFacts } from "../tools";
import { installNavigation, NavigationHost } from "./index";

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
  onMount(() => {
    setDocs([
      { id: 1, path: "C:\\Docs\\contrato.pdf", title: "contrato.pdf", sizes: Array.from({ length: 12 }, () => ({ width: 595, height: 842 })), revision: 0 },
      { id: 2, path: "C:\\Docs\\factura-octubre.pdf", title: "factura-octubre.pdf", sizes: [{ width: 595, height: 842 }], revision: 0 },
    ]);
    setActiveId(1);
    setDocFacts({ document: true });
    const offs = [
      registerMode({ id: "read", title: "Leer", icon: "mouse-pointer", order: 10, toolbar: () => <span class="pl-muted">Barra de Leer</span> }),
      registerStatusItem({ id: "demo.page", slot: "start", text: () => "Pág. 1 de 12" }),
      // Una herramienta nueva = un manifiesto + una línea de registro (sustituye a la fantasma con el mismo id).
      registerTool({ id: "optimize.compress", group: "compare-optimize", kind: "task", title: "tools.optimize.compress.title", description: "tools.optimize.compress.description",
        icon: "file-down", keywords: ["reducir", "tamaño"], when: { document: true }, pinnable: true, load: async () => ({ default: CompressPanel }) }),
      installNavigation(),
    ];
    setMode("read");
    onCleanup(() => offs.forEach((off) => off()));
  });
  return (
    <AppWindow tabs={[{ id: "1", title: "contrato.pdf" }, { id: "2", title: "factura-octubre.pdf" }]} activeId="1" onSelectTab={() => {}} onCloseTab={() => {}} onNewTab={() => {}} onReorderTabs={() => {}} appMenu={[]}>
      <NavigationHost />
      <EmptyState icon="file-text" title="Documento de ejemplo" hint="Prueba Ctrl+K, Ctrl+Mayús+A, Alt+1…9 y F1." />
    </AppWindow>
  );
}
