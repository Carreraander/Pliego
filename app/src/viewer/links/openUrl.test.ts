import { beforeEach, describe, expect, it, vi } from "vitest";

const openWebLink = vi.fn();
const toast = vi.fn();
vi.mock("../../api", () => ({ api: { openWebLink: (u: string) => openWebLink(u) } }));
vi.mock("../../ui", () => ({ toast: (o: unknown) => toast(o) }));

import { cancelOpenUrl, confirmOpenUrl, pendingUrl, requestOpenUrl, siteOf } from "./openUrl";

describe("abrir una dirección de un documento", () => {
  beforeEach(() => { openWebLink.mockReset(); toast.mockReset(); cancelOpenUrl(); });

  it("nunca se abre sin confirmar", async () => {
    requestOpenUrl("https://pdfpliego.com/");
    expect(pendingUrl()).toBe("https://pdfpliego.com/");
    expect(openWebLink).not.toHaveBeenCalled();
    await confirmOpenUrl();
    expect(openWebLink).toHaveBeenCalledWith("https://pdfpliego.com/");
    expect(pendingUrl()).toBeNull();
  });
  it("cancelar no abre nada", async () => {
    requestOpenUrl("mailto:hola@pdfpliego.com");
    cancelOpenUrl();
    await confirmOpenUrl();
    expect(openWebLink).not.toHaveBeenCalled();
  });
  it("rechaza ya en la interfaz lo que no es web ni correo", () => {
    for (const bad of ["file:///C:/x.exe", "javascript:alert(1)", "\\\\srv\\x", "ms-msdt:/id"]) requestOpenUrl(bad);
    expect(pendingUrl()).toBeNull();
    expect(toast).toHaveBeenCalledTimes(4);
  });
  it("avisa si el sistema no pudo abrirlo", async () => {
    openWebLink.mockRejectedValue("No se pudo abrir el enlace: x");
    requestOpenUrl("https://a.com");
    await confirmOpenUrl();
    expect(toast).toHaveBeenCalledWith(expect.objectContaining({ tone: "danger" }));
  });
  it("muestra el sitio aparte", () => {
    expect(siteOf("https://www.ejemplo.org/a/b?c")).toBe("www.ejemplo.org");
    expect(siteOf("mailto:yo@x.com?subject=a")).toBe("yo@x.com");
    expect(siteOf("nada")).toBe("");
  });
});
