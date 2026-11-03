import { createSignal, onCleanup, onMount } from "solid-js";
import { Chip, EmptyState, IconButton, Radio, RadioGroup, TaskPanel, ToggleGroup } from "../ui";
import { AppWindow, openInspector, registerMode, registerPanel, registerStatusItem, setMode, showNotice, type DocTab } from "./index";

/** Armazón con contenido de ejemplo para revisar el diseño (solo desarrollo): `/ui-shell`. */
export function ShellDemo() {
  const [tabs, setTabs] = createSignal<DocTab[]>([{ id: "a", title: "contrato.pdf", dirty: true }, { id: "b", title: "factura-octubre.pdf" }, { id: "c", title: "informe-anual-2026.pdf" }, { id: "home", title: "Inicio", kind: "screen" }]);
  const [active, setActive] = createSignal<string | null>("a");
  const [view, setView] = createSignal("continuous");

  const Compress = (p: { onClose: () => void }) => (
    <TaskPanel title="Comprimir" onClose={p.onClose} estimate="Ahora 12,4 MB → estimado ~3,1 MB" primary={{ label: "Guardar copia", onClick: () => {} }} secondary={[{ label: "Reemplazar…", onClick: () => {} }]}>
      <RadioGroup legend="Calidad" value="balanced"><Radio value="max" label="Máxima calidad" /><Radio value="balanced" label="Equilibrado" /><Radio value="min" label="Mínimo tamaño" /></RadioGroup>
    </TaskPanel>
  );

  onMount(() => {
    const offs = [
      registerMode({ id: "read", title: "Leer", icon: "mouse-pointer", order: 1, toolbar: () => (
        <>
          <IconButton icon="mouse-pointer" label="Seleccionar" pressed /><IconButton icon="hand" label="Mano" />
          <ToggleGroup label="Vista" value={view()} onChange={setView} options={[{ value: "continuous", icon: "layout-grid", title: "Continua" }, { value: "page", icon: "file-text", title: "Página" }]} />
          <IconButton icon="zoom-out" label="Alejar" /><Chip>100 %</Chip><IconButton icon="zoom-in" label="Acercar" /><IconButton icon="rotate-cw" label="Girar vista" />
        </>
      ) }),
      registerMode({ id: "comment", title: "Comentar", order: 2 }), registerMode({ id: "fill", title: "Rellenar y firmar", order: 3 }),
      registerMode({ id: "organize", title: "Organizar", order: 4 }), registerMode({ id: "edit", title: "Editar", order: 5 }), registerMode({ id: "protect", title: "Proteger", order: 6 }),
      registerPanel({ id: "demo.thumbs", title: "Miniaturas", icon: "layout-grid", order: 1, shortcut: "F4", component: () => <EmptyState title="Miniaturas" hint="Aquí van las páginas" /> }),
      registerPanel({ id: "demo.outline", title: "Marcadores", icon: "bookmark", order: 2, component: () => <EmptyState title="Marcadores" /> }),
      registerPanel({ id: "demo.comments", title: "Comentarios", icon: "message-square", order: 3, component: () => <EmptyState title="Sin comentarios" /> }),
      registerStatusItem({ id: "demo.page", slot: "start", hideOnHome: true, text: () => "Pág. 2 de 48 · A4 · 210 × 297 mm", label: "Página 2 de 48, A4", onClick: () => {} }),
      registerStatusItem({ id: "demo.task", slot: "center", text: () => "◷ OCR de informe.pdf 42 %", label: "OCR en curso, 42 por ciento", onClick: () => {} }),
      registerStatusItem({ id: "demo.sign", slot: "center", text: () => "✓ Firmado · válido", tone: "ok", label: "Firmado, válido", onClick: () => {} }),
      registerStatusItem({ id: "demo.zoom", slot: "end", text: () => "100 %", label: "Zoom 100 por ciento", onClick: () => {} }),
      showNotice("a", { kind: "signed", tone: "info", text: "Firmado por ANA GARCÍA · firma válida", actions: [{ label: "Ver firmas", onAction: () => {} }] }),
    ];
    setMode("read");
    openInspector(Compress);
    onCleanup(() => offs.forEach((off) => off()));
  });

  return (
    <AppWindow
      tabs={tabs()} activeId={active()} onSelectTab={setActive} onNewTab={() => {}}
      onCloseTab={(id) => setTabs((t) => t.filter((x) => x.id !== id))}
      onReorderTabs={(from, to) => setTabs((t) => { const n = [...t]; n.splice(to, 0, n.splice(from, 1)[0]!); return n; })}
      appMenu={[{ id: "open", label: "Abrir…", icon: "folder-open", shortcut: "Ctrl+O" }, { id: "settings", label: "Ajustes", icon: "settings", shortcut: "Ctrl+," }]}
    >
      <div class="demo-page"><div class="demo-paper" /></div>
    </AppWindow>
  );
}
