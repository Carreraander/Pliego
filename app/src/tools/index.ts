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
import { registerFormsTools } from "./forms/manifest";
import { registerSignTools } from "./sign/manifest";
import { registerProtectTools } from "./protect";
import { registerBatchTools } from "./batch/manifest";
import { registerViewTools } from "./view/manifest";
import { registerStampTools } from "./stamp/manifest";
import { registerFormTools } from "./formdesign/manifest";
import { registerEditTools } from "./edit/manifest";
import { registerMeasureTools } from "./measure/manifest";
import { registerOcrTools } from "./ocr/manifest";
import { registerCompareTools } from "./compare/manifest";
import { registerPrintTools } from "./print/manifest";

/** Herramientas reales conectadas a la app: cada flujo añade aquí una línea y sustituye a su fantasma de `base.ts`. */
export function registerAppTools(): void {
  registerOrganizeTools(); // W23
  registerOptimizeTools(); // W24
  registerCheckTools(); // W29
  registerAnnotateTools(); // W26
  registerFormsTools(); // W26
  registerSignTools(); // W28
  registerProtectTools(); // W25
  registerBatchTools(); // W29
  registerViewTools(); // W29
  registerConvertTools(); // W24
  registerStampTools(); // W27
  registerFormTools(); // W29b
  registerEditTools(); // W29b
  registerMeasureTools(); // W29
  registerOcrTools(); // W24
  registerCompareTools(); // W24
  registerPrintTools(); // W45
}
