import { scoped } from "../settings/text";

export const jt = scoped<string>({
  es: {
    "jobs.title": "Tareas", "jobs.close": "Cerrar", "jobs.none": "No hay tareas en curso.", "jobs.running": "En curso", "jobs.finished": "Terminadas", "jobs.failed": "Con errores",
    "jobs.status.one": "1 tarea en curso", "jobs.status.many": "{n} tareas en curso", "jobs.status.idle": "Tareas", "jobs.status.recent": "Tarea terminada",
    "jobs.cancel": "Cancelar", "jobs.open": "Abrir resultado", "jobs.reveal": "Mostrar en carpeta", "jobs.retry": "Reintentar", "jobs.dismiss": "Quitar de la lista", "jobs.clear": "Limpiar terminadas",
    "jobs.cancelled": "Cancelada", "jobs.done": "Terminada", "jobs.notify.done": "Tarea terminada", "jobs.notify.error": "La tarea falló", "jobs.progress": "{done} de {total}",
  },
  en: {
    "jobs.title": "Tasks", "jobs.close": "Close", "jobs.none": "No tasks running.", "jobs.running": "Running", "jobs.finished": "Finished", "jobs.failed": "Failed",
    "jobs.status.one": "1 task running", "jobs.status.many": "{n} tasks running", "jobs.status.idle": "Tasks", "jobs.status.recent": "Task finished",
    "jobs.cancel": "Cancel", "jobs.open": "Open result", "jobs.reveal": "Show in folder", "jobs.retry": "Retry", "jobs.dismiss": "Remove from list", "jobs.clear": "Clear finished",
    "jobs.cancelled": "Cancelled", "jobs.done": "Finished", "jobs.notify.done": "Task finished", "jobs.notify.error": "The task failed", "jobs.progress": "{done} of {total}",
  },
});
