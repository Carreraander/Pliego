import { createSignal, type JSX } from "solid-js";
import { Select, Switch, ToggleGroup } from "../ui";
import { settings, updateSettings, type Settings } from "./store";
import { st } from "./strings";

/** Una fila de ajuste: etiqueta, ayuda y control. `keywords` alimenta el buscador de ajustes. */
export interface SettingRow {
  id: string;
  label: () => string;
  hint?: () => string;
  keywords?: string[];
  control: () => JSX.Element;
}
export interface SettingsSection {
  id: string;
  title: () => string;
  icon: string;
  order?: number;
  rows?: SettingRow[];
  /** Contenido libre (Actualizaciones, Atajos, Acerca de). */
  component?: () => JSX.Element;
  /** Palabras del contenido libre para el buscador. */
  keywords?: string[];
}

const [extra, setExtra] = createSignal<SettingsSection[]>([]);
/** Los demás flujos añaden sus secciones (Comentarios, Firma, OCR e idiomas, Privacidad…) sin tocar esta carpeta. */
export function registerSettingsSection(section: SettingsSection): () => void {
  setExtra((all) => [...all.filter((s) => s.id !== section.id), section]);
  return () => setExtra((all) => all.filter((s) => s !== section));
}
export const extraSections = extra;

function choice<S extends keyof Settings, K extends keyof Settings[S] & string>(section: S, key: K, options: { value: string; label: () => string }[]): () => JSX.Element {
  return () => (
    <Select aria-label={st(`row.${section}.${key}`)} value={String(settings()[section][key])} options={options.map((o) => ({ value: o.value, label: o.label() }))}
      onChange={(e) => updateSettings(section, { [key]: e.currentTarget.value } as never)} />
  );
}
function toggle<S extends keyof Settings, K extends keyof Settings[S] & string>(section: S, key: K): () => JSX.Element {
  return () => <Switch aria-label={st(`row.${section}.${key}`)} label="" checked={Boolean(settings()[section][key])} onChange={(e) => updateSettings(section, { [key]: e.currentTarget.checked } as never)} />;
}

const row = (id: string, label: string, hint: string | null, control: () => JSX.Element, keywords: string[] = []): SettingRow =>
  ({ id, label: () => st(label), hint: hint ? () => st(hint) : undefined, control, keywords });

export const generalRows: SettingRow[] = [
  row("general.language", "row.general.language", "row.general.language.hint", choice("general", "language", [
    { value: "auto", label: () => st("opt.auto") }, { value: "es", label: () => "Español" }, { value: "en", label: () => "English" }]), ["idioma", "language"]),
  row("general.theme", "row.general.theme", null, choice("general", "theme", [
    { value: "auto", label: () => st("opt.system") }, { value: "light", label: () => st("opt.light") }, { value: "dark", label: () => st("opt.dark") }]), ["tema", "oscuro", "claro", "dark"]),
  row("general.density", "row.general.density", "row.general.density.hint", choice("general", "density", [
    { value: "comfortable", label: () => st("opt.comfortable") }, { value: "compact", label: () => st("opt.compact") }, { value: "touch", label: () => st("opt.touch") }]), ["densidad", "compacta", "táctil"]),
  row("general.reopenTabs", "row.general.reopenTabs", null, toggle("general", "reopenTabs"), ["pestañas", "reabrir", "sesión"]),
  row("general.greeting", "row.general.greeting", null, toggle("general", "greeting"), ["saludo", "inicio"]),
];

export const viewerRows: SettingRow[] = [
  row("viewer.defaultZoom", "row.viewer.defaultZoom", "row.viewer.defaultZoom.hint", choice("viewer", "defaultZoom", [
    { value: "fit-width", label: () => st("opt.fitWidth") }, { value: "fit-page", label: () => st("opt.fitPage") }, { value: "100", label: () => "100 %" }]), ["zoom", "ajustar"]),
  row("viewer.smoothScroll", "row.viewer.smoothScroll", null, toggle("viewer", "smoothScroll"), ["desplazamiento", "suave"]),
  row("viewer.nightReading", "row.viewer.nightReading", "row.viewer.nightReading.hint", toggle("viewer", "nightReading"), ["nocturna", "invertir", "oscuro"]),
];

export const accessibilityRows: SettingRow[] = [
  row("accessibility.reduceMotion", "row.accessibility.reduceMotion", "row.accessibility.reduceMotion.hint", choice("accessibility", "reduceMotion", [
    { value: "auto", label: () => st("opt.system") }, { value: "on", label: () => st("opt.on") }, { value: "off", label: () => st("opt.off") }]), ["movimiento", "animaciones"]),
  row("accessibility.strongFocus", "row.accessibility.strongFocus", "row.accessibility.strongFocus.hint", toggle("accessibility", "strongFocus"), ["foco", "teclado", "contorno"]),
];

export function UpdateModeToggle(props: { disabled?: boolean }) {
  return (
    <ToggleGroup disabled={props.disabled} label={st("row.updates.channel")} value={settings().updates.channel} onChange={(v) => updateSettings("updates", { channel: v as "stable" | "beta" })}
      options={[{ value: "stable", label: st("opt.stable") }, { value: "beta", label: st("opt.beta") }]} />
  );
}
