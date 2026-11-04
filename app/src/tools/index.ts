export * from "./registry";
export { addToolStrings } from "./strings";
export { registerBaseTools } from "./base";
export { registerToolIcons } from "./icons";
export { installOrganizeMode } from "./organize/mode";
export { installSignatures } from "./sign/facts";
import { registerOrganizeTools } from "./organize/manifest";
import { registerConvertTools } from "./convert/manifest";
import { registerOptimizeTools } from "./optimize/manifest";
import { registerCheckTools } from "./checks/manifest";
import { registerAnnotateTools } from "./annotate/manifest";
import { registerSignTools } from "./sign/manifest";
import { registerProtectTools } from "./protect";
import { registerBatchTools } from "./batch/manifest";
import { registerStampTools } from "./stamp/manifest";

/** Herramientas reales conectadas a la app: cada flujo añade aquí una línea y sustituye a su fantasma de `base.ts`. */
export function registerAppTools(): void {
  registerOrganizeTools(); // W23
  registerOptimizeTools(); // W24
  registerCheckTools(); // W29
  registerAnnotateTools(); // W26
  registerSignTools(); // W28
  registerProtectTools(); // W25
  registerBatchTools(); // W29
  registerConvertTools(); // W24
  registerStampTools(); // W27
}
