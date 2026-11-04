import { addToolStrings } from "../strings";

addToolStrings({
  es: {
    "checks.run": "Comprobar", "checks.running": "Comprobando…", "checks.page": "Página {n}",
    "checks.hint": "Se comprueba el archivo guardado en disco; no se modifica nada.",
    "checks.ok": "Sin problemas", "checks.result": "{errors} errores · {warnings} avisos",
    "checks.access.title": "Comprobar accesibilidad", "checks.pdfa.title": "Validar PDF/A", "checks.pdfa.level": "Nivel de PDF/A",
    "checks.preflight.title": "Comprobación previa", "checks.preflight.profile": "Perfil",
    "checks.profile.basic": "Básico (fuentes y líneas)", "checks.profile.print": "Imprenta (300 ppp)", "checks.profile.pdfx": "PDF/X (sin RGB)",
    "checks.failed": "No se pudo comprobar: {error}",
    "checks.order.title": "Orden de lectura", "checks.order.fix": "Ajustar tabulación a la estructura",
    "checks.order.hint": "Se muestra el orden en que un lector de pantalla recorre el documento. El ajuste guarda una copia; el original no cambia.",
    "checks.order.untagged": "El documento no tiene etiquetas de estructura, así que no hay orden de lectura. Pruébalo con «Comprobar accesibilidad».",
    "checks.order.backwards": "La lectura vuelve atrás en las páginas {pages}: revisa el orden.", "checks.order.noAlt": "Sin texto alternativo",
    "checks.order.suffix": "orden", "checks.order.job": "Orden de lectura de {name}", "checks.order.started": "Guardando copia con el orden ajustado",
  },
  en: {
    "checks.run": "Check", "checks.running": "Checking…", "checks.page": "Page {n}",
    "checks.hint": "The file saved on disk is checked; nothing is modified.",
    "checks.ok": "No problems found", "checks.result": "{errors} errors · {warnings} warnings",
    "checks.access.title": "Check accessibility", "checks.pdfa.title": "Validate PDF/A", "checks.pdfa.level": "PDF/A level",
    "checks.preflight.title": "Preflight", "checks.preflight.profile": "Profile",
    "checks.profile.basic": "Basic (fonts and lines)", "checks.profile.print": "Print (300 ppi)", "checks.profile.pdfx": "PDF/X (no RGB)",
    "checks.failed": "Could not check: {error}",
    "checks.order.title": "Reading order", "checks.order.fix": "Set tab order to structure",
    "checks.order.hint": "Shows the order a screen reader walks through the document. Fixing saves a copy; the original is not changed.",
    "checks.order.untagged": "The document has no structure tags, so there is no reading order. Try “Check accessibility”.",
    "checks.order.backwards": "Reading goes back on pages {pages}: review the order.", "checks.order.noAlt": "No alternative text",
    "checks.order.suffix": "order", "checks.order.job": "Reading order of {name}", "checks.order.started": "Saving a copy with the order fixed",
  },
});
