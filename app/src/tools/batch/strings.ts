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
    "batch.portfolio.title": "Carteras", "batch.portfolio.files": "Archivos de la cartera", "batch.portfolio.add": "Añadir archivos…",
    "batch.portfolio.remove": "Quitar", "batch.portfolio.create": "Crear cartera", "batch.portfolio.suffix": "cartera",
    "batch.portfolio.hint": "Se guarda una cartera nueva junto al primer archivo; los originales no cambian.",
    "batch.portfolio.started": "Creando la cartera… sigue en el centro de tareas", "batch.portfolio.job": "Cartera de {n} archivos",
    "batch.portfolio.inside": "Archivos dentro de este documento", "batch.portfolio.isPortfolio": "Este documento es una cartera.",
    "batch.portfolio.none": "Este documento no tiene archivos incrustados.", "batch.portfolio.extract": "Extraer todo",
    "batch.portfolio.extractSuffix": "adjuntos", "batch.portfolio.extractStarted": "Extrayendo archivos… sigue en el centro de tareas",
    "batch.portfolio.extractJob": "Extraer adjuntos de {name}", "batch.portfolio.listFailed": "No se pudo leer el documento: {error}",
  },
  en: {
    "batch.index.title": "Search index", "batch.index.folder": "PDF folder", "batch.index.pick": "Choose folder…",
    "batch.index.none": "No folder chosen yet", "batch.index.run": "Index",
    "batch.index.hint": "Subfolders are indexed too. Only changed files are read again.",
    "batch.index.started": "Indexing… follow it in the task centre", "batch.index.job": "Index {name}",
    "batch.index.search": "Search the index", "batch.index.go": "Search", "batch.index.empty": "No results",
    "batch.index.page": "{name}, page {n}", "batch.index.failed": "Could not search: {error}",
    "batch.portfolio.title": "Portfolios", "batch.portfolio.files": "Portfolio files", "batch.portfolio.add": "Add files…",
    "batch.portfolio.remove": "Remove", "batch.portfolio.create": "Create portfolio", "batch.portfolio.suffix": "portfolio",
    "batch.portfolio.hint": "A new portfolio is saved next to the first file; the originals do not change.",
    "batch.portfolio.started": "Creating the portfolio… follow it in the task centre", "batch.portfolio.job": "Portfolio of {n} files",
    "batch.portfolio.inside": "Files inside this document", "batch.portfolio.isPortfolio": "This document is a portfolio.",
    "batch.portfolio.none": "This document has no embedded files.", "batch.portfolio.extract": "Extract all",
    "batch.portfolio.extractSuffix": "attachments", "batch.portfolio.extractStarted": "Extracting files… follow it in the task centre",
    "batch.portfolio.extractJob": "Extract attachments from {name}", "batch.portfolio.listFailed": "Could not read the document: {error}",
  },
});

/** Textos del índice de búsqueda con sustitución de `{n}`. */
export function bt(key: string, vars: Record<string, string | number> = {}): string {
  return toolText(key, locale()).replace(/\{(\w+)\}/g, (_, n: string) => String(vars[n] ?? `{${n}}`));
}
