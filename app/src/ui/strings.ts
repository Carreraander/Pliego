import { locale, type Locale } from "../i18n";

/** Textos propios del armazón y los componentes (para no editar los diccionarios de las áreas). */
const dict = {
  es: {
    "ui.close": "Cerrar", "ui.closeNotice": "Cerrar aviso", "ui.back": "Atrás", "ui.next": "Siguiente",
    "ui.advanced": "Opciones avanzadas", "ui.step": "Paso {n} de {total}", "ui.cancel": "Cancelar", "ui.done": "Listo",
    "ui.progress": "Progreso", "ui.moreActions": "Más acciones",
  },
  en: {
    "ui.close": "Close", "ui.closeNotice": "Close notice", "ui.back": "Back", "ui.next": "Next",
    "ui.advanced": "Advanced options", "ui.step": "Step {n} of {total}", "ui.cancel": "Cancel", "ui.done": "Done",
    "ui.progress": "Progress", "ui.moreActions": "More actions",
  },
} as const satisfies Record<Locale, Record<string, string>>;

export type UiKey = keyof (typeof dict)["es"];

export function ut(key: UiKey, vars: Record<string, string | number> = {}): string {
  const text: string = dict[locale()][key] ?? dict.es[key];
  return text.replace(/\{(\w+)\}/g, (_, n: string) => String(vars[n] ?? `{${n}}`));
}
