import { scoped } from "../settings/text";

/** Textos de las autoactualizaciones (pastilla, popover, aviso de obligatoria y avisos de arranque). */
export const ut2 = scoped<string>({
  es: {
    "pill.ready": "Actualización lista", "pill.available": "Hay una versión nueva",
    "pop.title": "Pliego {version}", "pop.subtitle": "Lista para instalar", "pop.available": "Hay una versión nueva, sin descargar.",
    "pop.notes": "Novedades", "pop.noNotes": "Sin notas para esta versión.",
    "pop.restart": "Reiniciar ahora", "pop.restartElevated": "Reiniciar y actualizar (pide permiso de administrador)", "pop.later": "Al cerrar Pliego", "pop.download": "Descargar", "pop.skip": "Omitir esta versión",
    "pop.laterDone": "Se instalará la próxima vez que abras Pliego.", "pop.laterMachine": "Esta instalación es para todo el equipo: se actualiza con «Reiniciar y actualizar».",
    "pop.portable": "Esta copia es portátil: descarga la versión nueva desde el sitio de Pliego.",
    "mandatory": "Esta versión tiene un fallo de seguridad corregido en {version}.", "mandatory.action": "Reiniciar y actualizar", "mandatory.download": "Descargar",
    "unsaved.title": "Hay documentos con cambios sin guardar", "unsaved.text": "Para reiniciar hay que guardarlos primero:",
    "unsaved.save": "Guardar todo y reiniciar", "unsaved.cancel": "Cancelar",
    "rollback": "Se volvió a {restored} porque {failed} no arrancaba. No se volverá a instalar {failed}.",
    "restartFailed": "No se pudo reiniciar: {error}",
  },
  en: {
    "pill.ready": "Update ready", "pill.available": "A new version is available",
    "pop.title": "Pliego {version}", "pop.subtitle": "Ready to install", "pop.available": "A new version is available, not downloaded yet.",
    "pop.notes": "What’s new", "pop.noNotes": "No notes for this version.",
    "pop.restart": "Restart now", "pop.restartElevated": "Restart and update (asks for administrator permission)", "pop.later": "When I close Pliego", "pop.download": "Download", "pop.skip": "Skip this version",
    "pop.laterDone": "It will be installed the next time you open Pliego.", "pop.laterMachine": "This installation is for the whole PC: it updates with “Restart and update”.",
    "pop.portable": "This copy is portable: download the new version from the Pliego site.",
    "mandatory": "This version has a security flaw fixed in {version}.", "mandatory.action": "Restart and update", "mandatory.download": "Download",
    "unsaved.title": "Some documents have unsaved changes", "unsaved.text": "Save them first to restart:",
    "unsaved.save": "Save all and restart", "unsaved.cancel": "Cancel",
    "rollback": "Went back to {restored} because {failed} did not start. {failed} will not be installed again.",
    "restartFailed": "Could not restart: {error}",
  },
});
