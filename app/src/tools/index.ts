export * from "./registry";
export { addToolStrings } from "./strings";
export { registerBaseTools } from "./base";
export { registerToolIcons } from "./icons";
import { registerOptimizeTools } from "./optimize/manifest";

/** Herramientas reales conectadas a la app: cada flujo añade aquí una línea y sustituye a su fantasma de `base.ts`. */
export function registerAppTools(): void {
  registerOptimizeTools(); // W24
}
