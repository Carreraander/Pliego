import { createSignal, For, type JSX } from "solid-js";
import {
  ContextMenu, Menu, TabPanel, Tabs, Tooltip, toast, ToastRegion, contextPoint, type MenuEntry, type Point,
  Inspector, TaskPanel, WizardSteps,
  Button, Checkbox, Chip, EmptyState, Field, Icon, IconButton, Kbd, Logo, Notice, Radio, RadioGroup, Select, Slider, Switch,
  TextInput, ToggleGroup,
} from "./index";
import "./Catalog.css";

/** Página de revisión visual de todos los componentes y estados. Solo en desarrollo: `/ui-catalog`. */
export function Catalog() {
  const [theme, setTheme] = createSignal("auto");
  const [density, setDensity] = createSignal("comfortable");
  const [view, setView] = createSignal("continuous");
  const [quality, setQuality] = createSignal("balanced");
  const [zoom, setZoom] = createSignal("50");
  const [menuOpen, setMenuOpen] = createSignal(false);
  const [menuAnchor, setMenuAnchor] = createSignal<HTMLElement>();
  const [ctx, setCtx] = createSignal<Point | null>(null);
  const [tab, setTab] = createSignal("props");
  const [wrap, setWrap] = createSignal(true);
  const [running, setRunning] = createSignal(false);
  const items = (): MenuEntry[] => [
    { id: "open", label: "Abrir…", icon: "folder-open", shortcut: "Ctrl+O" },
    { id: "new", label: "Nuevo", icon: "file-plus", items: [{ id: "n1", label: "Desde archivos" }, { id: "n2", label: "Desde el portapapeles" }] },
    { type: "separator" },
    { type: "label", label: "Vista" },
    { id: "wrap", label: "Ajustar líneas", checked: wrap(), onSelect: () => setWrap(!wrap()) },
    { id: "sign", label: "Validar firmas", disabled: true, reason: "Este documento no tiene firmas" },
    { type: "separator" },
    { id: "del", label: "Eliminar", icon: "trash", danger: true },
  ];

  const apply = (name: "theme" | "density", v: string) => {
    const el = document.documentElement;
    if (v === "auto" || v === "comfortable") el.removeAttribute(`data-${name}`);
    else el.setAttribute(`data-${name}`, v);
    (name === "theme" ? setTheme : setDensity)(v);
  };

  return (
    <div class="catalog">
      <header class="catalog__head">
        <Logo size={28} />
        <h1>Catálogo de componentes</h1>
        <ToggleGroup label="Tema" value={theme()} onChange={(v) => apply("theme", v)}
          options={[{ value: "auto", label: "Sistema" }, { value: "light", label: "Claro" }, { value: "dark", label: "Oscuro" }]} />
        <ToggleGroup label="Densidad" value={density()} onChange={(v) => apply("density", v)}
          options={[{ value: "compact", label: "Compacta" }, { value: "comfortable", label: "Cómoda" }, { value: "touch", label: "Táctil" }]} />
      </header>

      <Section title="Botones" id="buttons">
        <For each={["primary", "normal", "ghost", "danger"] as const}>{(v) => (
          <div class="catalog__row">
            <Button variant={v}>{v}</Button>
            <Button variant={v} icon="plus">Con icono</Button>
            <Button variant={v} disabled>Desactivado</Button>
            <Button variant={v} busy>Ocupado</Button>
          </div>
        )}</For>
        <div class="catalog__row">
          <IconButton icon="search" label="Buscar" />
          <IconButton icon="star" label="Fijar" pressed />
          <IconButton icon="trash" label="Eliminar" variant="danger" />
          <IconButton icon="more" label="Más" disabled />
          <IconButton icon="x" label="Cerrar" size="sm" />
        </div>
      </Section>

      <Section title="Iconos" id="icons">
        <div class="catalog__icons">
          <For each={["redact", "bates", "pdf-a", "ocr", "timestamp", "sign-digital", "highlighter", "stamp", "lock", "layers"]}>{(n) => (
            <span class="catalog__icon"><Icon name={n} /><small>{n}</small></span>
          )}</For>
        </div>
      </Section>

      <Section title="Campos" id="fields">
        <div class="catalog__grid">
          <Field label="Nombre" hint="Cómo se verá en la firma"><TextInput placeholder="Ana García" /></Field>
          <Field label="Contraseña" error="La contraseña no es correcta" required><TextInput type="password" value="abc" /></Field>
          <Field label="Desactivado"><TextInput disabled value="No editable" /></Field>
          <Field label="Idioma de OCR">
            <Select value="es" options={[{ value: "es", label: "Español" }, { value: "en", label: "Inglés" }, { value: "fr", label: "Francés", disabled: true }]} />
          </Field>
          <Field label="Zoom"><Slider min={10} max={400} value={zoom()} valueLabel={`${zoom()} %`} onInput={(e) => setZoom(e.currentTarget.value)} /></Field>
        </div>
      </Section>

      <Section title="Selección" id="choice">
        <div class="catalog__grid">
          <div class="catalog__stack">
            <Checkbox label="Marcada" checked /><Checkbox label="Sin marcar" /><Checkbox label="Indeterminada" indeterminate />
            <Checkbox label="Desactivada" disabled /><Checkbox label="Con ayuda" hint="Texto de ayuda en segundo plano" />
          </div>
          <RadioGroup legend="Calidad" value={quality()} onChange={setQuality}>
            <Radio value="max" label="Máxima calidad" /><Radio value="balanced" label="Equilibrado" hint="Recomendado" /><Radio value="min" label="Mínimo tamaño" />
          </RadioGroup>
          <div class="catalog__stack">
            <Switch label="Activado" checked /><Switch label="Desactivado" /><Switch label="Bloqueado" disabled />
          </div>
          <ToggleGroup label="Vista" value={view()} onChange={setView} options={[
            { value: "continuous", icon: "layout-grid", title: "Continua" }, { value: "page", icon: "file-text", title: "Página" },
            { value: "double", icon: "layers", title: "Doble" }]} />
        </div>
      </Section>

      <Section title="Menús, tooltip, pestañas y avisos emergentes" id="overlays">
        <div class="catalog__row">
          <Button ref={setMenuAnchor} id="menu-btn" icon="chevron-down" aria-haspopup="menu" aria-expanded={menuOpen()} onClick={() => setMenuOpen(!menuOpen())}>Menú</Button>
          <Menu anchor={menuAnchor()} open={menuOpen()} onClose={() => setMenuOpen(false)} items={items()} label="Menú de ejemplo" />
          <div class="catalog__ctx" id="ctx-area" tabindex={0} onContextMenu={(e) => setCtx(contextPoint(e))}>Clic derecho aquí</div>
          <ContextMenu point={ctx()} onClose={() => setCtx(null)} items={items()} label="Menú contextual" />
          <Tooltip text="Buscar" shortcut="Ctrl+K"><IconButton icon="search" label="Buscar" id="tip-btn" /></Tooltip>
          <Button onClick={() => toast({ tone: "ok", text: "Copia guardada" })}>Aviso</Button>
          <Button onClick={() => toast({ tone: "danger", text: "No se pudo guardar", action: { label: "Reintentar", onAction: () => {} } })}>Error</Button>
        </div>
        <Tabs label="Secciones" value={tab()} onChange={setTab} tabs={[{ id: "props", label: "Propiedades" }, { id: "comments", label: "Comentarios", icon: "message-square" }, { id: "off", label: "Desactivada", disabled: true }]}>
          <TabPanel id="props"><p>Contenido de propiedades</p></TabPanel>
          <TabPanel id="comments"><p>Contenido de comentarios</p></TabPanel>
        </Tabs>
      </Section>

      <Section title="Inspector, panel de tarea y asistente" id="panels">
        <WizardSteps steps={["Dónde", "Con qué", "Opciones", "Firmar"]} current={1} />
        <div class="catalog__panels">
          <div class="catalog__panel">
            <TaskPanel title="Comprimir" onClose={() => {}} estimate="Ahora 12,4 MB → estimado ~3,1 MB"
              primary={{ label: "Guardar copia", onClick: () => setRunning(true) }} secondary={[{ label: "Reemplazar…", onClick: () => {} }]}
              advanced={<Checkbox label="Quitar metadatos" />}>
              <RadioGroup legend="Calidad" value="balanced"><Radio value="max" label="Máxima calidad" /><Radio value="balanced" label="Equilibrado" /><Radio value="min" label="Mínimo tamaño" /></RadioGroup>
            </TaskPanel>
          </div>
          <div class="catalog__panel">
            <TaskPanel title="Reconocer texto (OCR)" onClose={() => {}} primary={{ label: "Guardar copia", onClick: () => {} }}
              progress={running() ? { value: 0.42, label: "Reconociendo página 21 de 48", onCancel: () => setRunning(false) } : { value: null, label: "Preparando…", onCancel: () => {} }}
              notice={<Notice inline tone="warn">Este documento está firmado: se guardará como copia.</Notice>}>
              <Field label="Idioma"><Select value="es" options={[{ value: "es", label: "Español" }]} /></Field>
            </TaskPanel>
          </div>
          <div class="catalog__panel">
            <Inspector title="Propiedades" onClose={() => {}}><Field label="Autor"><TextInput value="Ana García" /></Field><Slider min={0} max={100} value={60} valueLabel="60 %" aria-label="Opacidad" /></Inspector>
          </div>
        </div>
      </Section>

      <Section title="Avisos, chips y atajos" id="feedback">
        <Notice tone="info" actions={[{ label: "Ver firmas", onAction: () => {} }]}>Firmado por ANA GARCÍA · firma válida</Notice>
        <Notice tone="ok">Se guardó la copia «contrato (comprimido).pdf»</Notice>
        <Notice tone="warn" actions={[{ label: "Guardar versión reparada", onAction: () => {} }]} onDismiss={() => {}}>Se reparó al abrir</Notice>
        <Notice tone="danger">No se pudo abrir «informe.pdf»: el archivo está dañado.</Notice>
        <div class="catalog__row">
          <Chip>Neutro</Chip><Chip tone="accent">Acento</Chip><Chip tone="ok" icon="check">Firmado</Chip><Chip tone="warn">Beta</Chip>
          <Chip tone="danger">Error</Chip><Chip tone="info">Info</Chip><Chip onClick={() => {}} icon="lock">Protegido</Chip>
          <Kbd keys="Ctrl+K" /><Kbd keys="Ctrl+Mayús+S" />
        </div>
        <div class="catalog__empty"><EmptyState icon="file-text" title="Abre un PDF para empezar" hint="Arrastra un archivo aquí o usa Abrir."><Button variant="primary">Abrir…</Button></EmptyState></div>
      </Section>
      <ToastRegion />
    </div>
  );
}

function Section(props: { title: string; id: string; children: JSX.Element }) {
  return (
    <section class="catalog__section" aria-labelledby={`h-${props.id}`}>
      <h2 id={`h-${props.id}`}>{props.title}</h2>
      {props.children}
    </section>
  );
}
