import { registerPanel } from "../../shell";
import { AssistantPanel } from "./AssistantPanel";
import { at } from "./strings";

/** Registra el panel «Asistente» en el raíl lateral. Devuelve cómo quitarlo. */
export function registerAssistantPanel(): () => void {
  return registerPanel({
    id: "assistant.panel",
    title: at("title"),
    icon: "message-square",
    order: 80,
    component: AssistantPanel,
  });
}

export { AssistantPanel };
