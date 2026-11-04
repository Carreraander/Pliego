export * from "./registry";
export { addToolStrings } from "./strings";
export { registerBaseTools } from "./base";
export { registerToolIcons } from "./icons";
export { installSignatures } from "./sign/facts";
import { registerOrganizeTools } from "./organize/manifest";
import { registerConvertTools } from "./convert/manifest";
import { registerOptimizeTools } from "./optimize/manifest";
import { registerSignTools } from "./sign/manifest";
import { registerStampTools } from "./stamp/manifest";

/** Herramientas reales conectadas a la app: cada flujo añade aquí una línea y sustituye a su fantasma de `base.ts`. */
export function registerAppTools(): void {
  registerOrganizeTools(); // W23
  registerOptimizeTools(); // W24
  registerSignTools(); // W28
  registerConvertTools(); // W24
  registerStampTools(); // W27
}
