import { addToolStrings, toolText } from "../strings";
import { locale } from "../../i18n";

addToolStrings({
  es: {
    "compare.suffix": "comparación",
    "compare.title": "Comparar documentos",
    "compare.other": "Versión nueva", "compare.other.pick": "Elegir el otro PDF…",
    "compare.other.none": "Aún no has elegido el documento con el que comparar",
    "compare.other.hint": "Se compara el documento abierto (versión original) con este (versión nueva).",
    "compare.text": "Comparar el texto, palabra por palabra", "compare.visual": "Comparar el aspecto de las páginas",
    "compare.same": "Elige un documento distinto del abierto.",
    "compare.run": "Comparar y guardar informe",
    "compare.note": "Los dos documentos quedan como están: el informe se guarda como «{name}».",
    "compare.started": "Comparando… sigue en el centro de tareas", "compare.job": "Comparar {name}",
    "compare.noDoc": "Abre el documento original para compararlo con otro.",
  },
  en: {
    "compare.suffix": "comparison",
    "compare.title": "Compare documents",
    "compare.other": "New version", "compare.other.pick": "Choose the other PDF…",
    "compare.other.none": "You haven’t chosen the document to compare with yet",
    "compare.other.hint": "The open document (original version) is compared with this one (new version).",
    "compare.text": "Compare the text, word by word", "compare.visual": "Compare how the pages look",
    "compare.same": "Choose a document other than the open one.",
    "compare.run": "Compare and save a report",
    "compare.note": "Both documents stay as they are: the report is saved as “{name}”.",
    "compare.started": "Comparing… follow it in the task center", "compare.job": "Compare {name}",
    "compare.noDoc": "Open the original document to compare it with another.",
  },
});

export function cmt(key: string, vars: Record<string, string | number> = {}): string {
  return toolText(key, locale()).replace(/\{(\w+)\}/g, (_, n: string) => String(vars[n] ?? `{${n}}`));
}
