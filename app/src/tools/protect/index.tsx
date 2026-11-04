/**
 * Proteger y Redactar en la app (W25): modo `protect.protect` con su barra, y la tarea `protect.sanitize`.
 * Los paneles se cargan de forma perezosa; la lógica de las peticiones vive en `request.ts` (con tests).
 */
import { lazy } from "solid-js";
import { registerMode } from "../../shell/slots";
import { getTool, registerTool, toolTitle } from "../registry";

// La barra se carga al mostrarla: así este archivo (y su test) no arrastra la interfaz.
const Toolbar = lazy(() => import("./Toolbar"));

/** Registra las herramientas de Proteger y devuelve cómo retirarlas. */
export function registerProtectTools(): () => void {
  const offs = [
    registerTool({
      id: "protect.protect", group: "protect", kind: "mode", modeId: "protect.protect",
      title: "tools.protect.protect.title", description: "tools.protect.protect.description", icon: "lock",
      keywords: ["contraseña", "cifrar", "permisos", "aes"], when: { document: true }, pinnable: true,
    }),
    registerTool({
      id: "protect.sanitize", group: "protect", kind: "task",
      title: "tools.protect.sanitize.title", description: "tools.protect.sanitize.description", icon: "eraser",
      keywords: ["sanear", "limpiar", "metadatos"], when: { document: true }, pinnable: true,
      load: () => import("./SanitizePanel"),
    }),
    registerMode({
      id: "protect.protect", icon: "lock", order: 60, needsDocument: true,
      get title() { const t = getTool("protect.protect"); return t ? toolTitle(t) : "protect.protect"; },
      toolbar: () => <Toolbar />,
    }),
  ];
  return () => offs.forEach((off) => off());
}
