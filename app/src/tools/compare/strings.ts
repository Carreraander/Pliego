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
    "compare.sideBySide": "Ver lado a lado", "compare.sideBySide.busy": "Comparando…",
    "compare.sideBySide.hint": "Los dos documentos se enseñan juntos, con el mismo desplazamiento y los cambios resaltados.",
    "view.title": "Comparar lado a lado", "view.original": "Original", "view.newer": "Nueva",
    "view.prev": "Cambio anterior (Mayús+F3)", "view.next": "Cambio siguiente (F3)", "view.none": "Sin cambios",
    "view.count": "{n} de {total}", "view.row": "Fila {n} de {total}",
    "view.showText": "Resaltar el texto", "view.showVisual": "Marcar el aspecto", "view.legend": "Leyenda",
    "view.kind.removed": "Quitado", "view.kind.added": "Añadido", "view.kind.changed": "Cambiado", "view.kind.visual": "Aspecto distinto",
    "view.zoomIn": "Acercar", "view.zoomOut": "Alejar", "view.fit": "Ajustar a la ventana", "view.close": "Cerrar la comparación",
    "view.gap.inserted": "Página nueva: no existe en el original", "view.gap.deleted": "Página eliminada: ya no está en la versión nueva",
    "view.summary": "{pages} página(s) con cambios · {inserted} insertada(s) · {deleted} eliminada(s) · palabras +{added} −{removed} ~{changed}",
    "view.failed": "No se pudo comparar", "view.tab": "Comparación",
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
    "compare.sideBySide": "View side by side", "compare.sideBySide.busy": "Comparing…",
    "compare.sideBySide.hint": "Both documents are shown together, scrolling as one, with the changes highlighted.",
    "view.title": "Compare side by side", "view.original": "Original", "view.newer": "New",
    "view.prev": "Previous change (Shift+F3)", "view.next": "Next change (F3)", "view.none": "No changes",
    "view.count": "{n} of {total}", "view.row": "Row {n} of {total}",
    "view.showText": "Highlight text", "view.showVisual": "Mark appearance", "view.legend": "Legend",
    "view.kind.removed": "Removed", "view.kind.added": "Added", "view.kind.changed": "Changed", "view.kind.visual": "Looks different",
    "view.zoomIn": "Zoom in", "view.zoomOut": "Zoom out", "view.fit": "Fit to window", "view.close": "Close the comparison",
    "view.gap.inserted": "New page: it isn’t in the original", "view.gap.deleted": "Deleted page: it’s no longer in the new version",
    "view.summary": "{pages} page(s) changed · {inserted} inserted · {deleted} deleted · words +{added} −{removed} ~{changed}",
    "view.failed": "Couldn’t compare", "view.tab": "Comparison",
  },
});

export function cmt(key: string, vars: Record<string, string | number> = {}): string {
  return toolText(key, locale()).replace(/\{(\w+)\}/g, (_, n: string) => String(vars[n] ?? `{${n}}`));
}
