import type { Locale } from "../i18n";

/** Textos de las herramientas por idioma. Un flujo añade los suyos con `addToolStrings`. */
const dict: Record<Locale, Record<string, string>> = {
  es: {
    "reason.soon": "Disponible más adelante",
    "reason.document": "Abre un documento para usarla",
    "reason.forms": "Este documento no tiene campos de formulario",
    "reason.signatures": "Este documento no tiene firmas",
    "reason.protected": "Solo para documentos protegidos",
    "reason.notProtected": "El documento ya está protegido",
    "reason.pdfa": "Solo para documentos PDF/A",
    "group.view": "Ver", "group.comment": "Comentar", "group.fill-sign": "Rellenar y firmar", "group.organize": "Organizar",
    "group.edit": "Editar", "group.forms": "Formularios", "group.scan-ocr": "Escanear y OCR", "group.protect": "Proteger",
    "group.convert": "Convertir", "group.compare-optimize": "Comparar y optimizar", "group.standards": "Estándares y accesibilidad",
    "group.print": "Impresión", "group.automate": "Automatizar", "group.assistant": "Asistente",
  },
  en: {
    "reason.soon": "Available later",
    "reason.document": "Open a document to use it",
    "reason.forms": "This document has no form fields",
    "reason.signatures": "This document has no signatures",
    "reason.protected": "Only for protected documents",
    "reason.notProtected": "The document is already protected",
    "reason.pdfa": "Only for PDF/A documents",
    "group.view": "View", "group.comment": "Comment", "group.fill-sign": "Fill and sign", "group.organize": "Organize",
    "group.edit": "Edit", "group.forms": "Forms", "group.scan-ocr": "Scan and OCR", "group.protect": "Protect",
    "group.convert": "Convert", "group.compare-optimize": "Compare and optimize", "group.standards": "Standards and accessibility",
    "group.print": "Print", "group.automate": "Automate", "group.assistant": "Assistant",
  },
};

export function addToolStrings(strings: Partial<Record<Locale, Record<string, string>>>): void {
  for (const loc of Object.keys(strings) as Locale[]) Object.assign(dict[loc], strings[loc]);
}

/** Traduce una clave de herramienta; si falta en el idioma usa español y, si no existe, devuelve la propia clave. */
export function toolText(key: string, loc: Locale): string {
  return dict[loc][key] ?? dict.es[key] ?? key;
}
