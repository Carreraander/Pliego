import { addToolStrings } from "../strings";
import { locale } from "../../i18n";
import { toolText } from "../strings";

addToolStrings({
  es: {
    "view.props.title": "Propiedades del documento", "view.props.noDoc": "Abre un documento para ver sus propiedades",
    "view.props.title.row": "Título", "view.props.author": "Autor", "view.props.subject": "Asunto", "view.props.keywords": "Palabras clave",
    "view.props.creator": "Creado con", "view.props.producer": "Generado por", "view.props.format": "Formato", "view.props.pages": "Páginas",
    "view.props.pageSize": "Tamaño de página", "view.props.fileSize": "Tamaño del archivo", "view.props.modified": "Modificado",
    "view.props.encryption": "Cifrado", "view.props.path": "Ubicación", "view.props.failed": "No se pudieron leer las propiedades: {error}",
    "view.props.show": "Mostrar en carpeta",
  },
  en: {
    "view.props.title": "Document properties", "view.props.noDoc": "Open a document to see its properties",
    "view.props.title.row": "Title", "view.props.author": "Author", "view.props.subject": "Subject", "view.props.keywords": "Keywords",
    "view.props.creator": "Created with", "view.props.producer": "Produced by", "view.props.format": "Format", "view.props.pages": "Pages",
    "view.props.pageSize": "Page size", "view.props.fileSize": "File size", "view.props.modified": "Modified",
    "view.props.encryption": "Encryption", "view.props.path": "Location", "view.props.failed": "Could not read the properties: {error}",
    "view.props.show": "Show in folder",
  },
});

/** Textos de las herramientas de Ver con sustitución de `{n}`. */
export function vt(key: string, vars: Record<string, string | number> = {}): string {
  return toolText(key, locale()).replace(/\{(\w+)\}/g, (_, n: string) => String(vars[n] ?? `{${n}}`));
}
