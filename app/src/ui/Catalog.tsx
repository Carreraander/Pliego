import { createSignal, For, type JSX } from "solid-js";
import {
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
