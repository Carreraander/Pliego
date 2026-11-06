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
    "view.props.show": "Mostrar en carpeta", "view.props.save": "Guardar cambios", "view.props.undo": "Deshacer",
    "view.props.hint": "Se guarda en el propio PDF; antes se deja una copia «(antes de las propiedades)».",
    "view.props.backup": "antes de las propiedades", "view.props.saved": "Propiedades guardadas en el PDF.",
    "view.props.saveFailed": "No se pudieron guardar las propiedades: {error}",
    "view.att.title": "Adjuntos", "view.att.none": "Este documento no tiene archivos adjuntos.", "view.att.noDoc": "Abre un documento para ver sus adjuntos",
    "view.att.extract": "Extraer", "view.att.extractAll": "Extraer todos", "view.att.suffix": "adjuntos",
    "view.att.hint": "Extraer guarda los adjuntos en una carpeta nueva junto al documento. Añadir y quitar cambian el PDF; antes se deja una copia «(antes de los adjuntos)».",
    "view.att.add": "Añadir archivos…", "view.att.pick": "Archivos para adjuntar", "view.att.remove": "Quitar", "view.att.undo": "Deshacer",
    "view.att.backup": "antes de los adjuntos", "view.att.added": "{n} archivo(s) adjuntados.", "view.att.removed": "Adjunto «{name}» quitado.",
    "view.att.editFailed": "No se pudieron cambiar los adjuntos: {error}",
    "view.att.started": "Extrayendo… sigue en el centro de tareas", "view.att.job": "Extraer adjuntos de {name}",
    "view.att.failed": "No se pudieron leer los adjuntos: {error}",
    "view.layers.title": "Capas", "view.layers.none": "Este documento no tiene capas.", "view.layers.noDoc": "Abre un documento para ver sus capas",
    "view.layers.showAll": "Mostrar todas", "view.layers.failed": "No se pudieron cambiar las capas: {error}",
    "view.layers.hint": "Mostrar u ocultar capas solo cambia lo que ves; el archivo no se modifica.",
  },
  en: {
    "view.props.title": "Document properties", "view.props.noDoc": "Open a document to see its properties",
    "view.props.title.row": "Title", "view.props.author": "Author", "view.props.subject": "Subject", "view.props.keywords": "Keywords",
    "view.props.creator": "Created with", "view.props.producer": "Produced by", "view.props.format": "Format", "view.props.pages": "Pages",
    "view.props.pageSize": "Page size", "view.props.fileSize": "File size", "view.props.modified": "Modified",
    "view.props.encryption": "Encryption", "view.props.path": "Location", "view.props.failed": "Could not read the properties: {error}",
    "view.props.show": "Show in folder", "view.props.save": "Save changes", "view.props.undo": "Undo",
    "view.props.hint": "Saved in the PDF itself; a copy “(before properties)” is kept first.",
    "view.props.backup": "before properties", "view.props.saved": "Properties saved in the PDF.",
    "view.props.saveFailed": "Could not save the properties: {error}",
    "view.att.title": "Attachments", "view.att.none": "This document has no attached files.", "view.att.noDoc": "Open a document to see its attachments",
    "view.att.extract": "Extract", "view.att.extractAll": "Extract all", "view.att.suffix": "attachments",
    "view.att.hint": "Extracting saves the attachments in a new folder next to the document. Adding and removing change the PDF; a copy “(before attachments)” is kept first.",
    "view.att.add": "Add files…", "view.att.pick": "Files to attach", "view.att.remove": "Remove", "view.att.undo": "Undo",
    "view.att.backup": "before attachments", "view.att.added": "{n} file(s) attached.", "view.att.removed": "Attachment “{name}” removed.",
    "view.att.editFailed": "Could not change the attachments: {error}",
    "view.att.started": "Extracting… follow it in the task centre", "view.att.job": "Extract attachments from {name}",
    "view.att.failed": "Could not read the attachments: {error}",
    "view.layers.title": "Layers", "view.layers.none": "This document has no layers.", "view.layers.noDoc": "Open a document to see its layers",
    "view.layers.showAll": "Show all", "view.layers.failed": "Could not change the layers: {error}",
    "view.layers.hint": "Showing or hiding layers only changes what you see; the file is not modified.",
  },
});

/** Textos de las herramientas de Ver con sustitución de `{n}`. */
export function vt(key: string, vars: Record<string, string | number> = {}): string {
  return toolText(key, locale()).replace(/\{(\w+)\}/g, (_, n: string) => String(vars[n] ?? `{${n}}`));
}
