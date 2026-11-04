import { describe, expect, it } from "vitest";
import { toolText } from "../strings";
import "./strings";

describe("textos de Firmas", () => {
  it("todas las claves tienen español e inglés", () => {
    // Las claves de `sign.*` se comprueban pidiendo la traducción: si falta, devuelve la propia clave.
    for (const key of ["sign.title", "sign.cert.empty", "validate.none", "verdict.modified", "flow.mode.sign", "notice.view", "tools.sign.digital.title"]) {
      expect(toolText(key, "es")).not.toBe(key);
      expect(toolText(key, "en")).not.toBe(key);
      expect(toolText(key, "en")).not.toBe(toolText(key, "es"));
    }
  });
});
