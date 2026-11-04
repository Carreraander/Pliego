import { addToolStrings } from "../strings";
import { locale } from "../../i18n";
import { toolText } from "../strings";

addToolStrings({
  es: {
    "batch.index.title": "Índice de búsqueda", "batch.index.folder": "Carpeta de PDF", "batch.index.pick": "Elegir carpeta…",
    "batch.index.none": "Aún no has elegido ninguna carpeta", "batch.index.run": "Indexar",
    "batch.index.hint": "Indexa también las subcarpetas. Solo se vuelven a leer los archivos que cambiaron.",
    "batch.index.started": "Indexando… sigue en el centro de tareas", "batch.index.job": "Indexar {name}",
    "batch.index.search": "Buscar en el índice", "batch.index.go": "Buscar", "batch.index.empty": "Sin resultados",
    "batch.index.page": "{name}, página {n}", "batch.index.failed": "No se pudo buscar: {error}",
  },
  en: {
    "batch.index.title": "Search index", "batch.index.folder": "PDF folder", "batch.index.pick": "Choose folder…",
    "batch.index.none": "No folder chosen yet", "batch.index.run": "Index",
    "batch.index.hint": "Subfolders are indexed too. Only changed files are read again.",
    "batch.index.started": "Indexing… follow it in the task centre", "batch.index.job": "Index {name}",
    "batch.index.search": "Search the index", "batch.index.go": "Search", "batch.index.empty": "No results",
    "batch.index.page": "{name}, page {n}", "batch.index.failed": "Could not search: {error}",
  },
});

/** Textos del índice de búsqueda con sustitución de `{n}`. */
export function bt(key: string, vars: Record<string, string | number> = {}): string {
  return toolText(key, locale()).replace(/\{(\w+)\}/g, (_, n: string) => String(vars[n] ?? `{${n}}`));
}
