/** Construcción (pura, con pruebas) de los argumentos de los comandos de firma. */

export type Level = "b-b" | "b-t" | "b-lt" | "b-lta";
export type Corner = "bottom-left" | "bottom-right" | "top-left" | "top-right";

export interface KeyChoice {
  source: "store" | "pfx";
  thumbprint: string;
  pfxPath: string;
  password: string;
}

export interface SignForm {
  key: KeyChoice;
  level: Level;
  tsa: string;
  visible: boolean;
  /** Página (desde 1). */
  page: number;
  corner: Corner;
  /** `0` = firma normal; 1–3 = firma de certificación (DocMDP). */
  certify: 0 | 1 | 2 | 3;
  reason: string;
  location: string;
  contact: string;
}

export const emptyKey = (): KeyChoice => ({ source: "store", thumbprint: "", pfxPath: "", password: "" });

export const needsTimestamp = (level: Level): boolean => level !== "b-b";

/** Tamaño de la firma visible, en puntos. */
export const VISIBLE_SIZE = { width: 200, height: 60 } as const;
const MARGIN = 36;

/** Rectángulo `[x, y, ancho, alto]` en puntos, origen abajo a la izquierda (como pide `pliego-sign`). */
export function visibleRect(page: { width: number; height: number }, corner: Corner): [number, number, number, number] {
  const { width: w, height: h } = VISIBLE_SIZE;
  const x = corner.endsWith("left") ? MARGIN : Math.max(0, page.width - w - MARGIN);
  const y = corner.startsWith("bottom") ? MARGIN : Math.max(0, page.height - h - MARGIN);
  return [x, y, w, h];
}

/** `KeySource` del servicio (`kind` en snake_case) o `null` si falta algo. */
export function keySource(k: KeyChoice): Record<string, string> | null {
  if (k.source === "store") return k.thumbprint ? { kind: "store", thumbprint: k.thumbprint } : null;
  return k.pfxPath ? { kind: "pfx", path: k.pfxPath, password: k.password } : null;
}

/** Por qué no se puede firmar todavía, como clave de texto; `null` si está todo. */
export function missing(f: SignForm): "key" | "tsa" | "page" | null {
  if (!keySource(f.key)) return "key";
  if (needsTimestamp(f.level) && !/^https?:\/\//i.test(f.tsa.trim())) return "tsa";
  if (f.visible && (!Number.isInteger(f.page) || f.page < 1)) return "page";
  return null;
}

const text = (s: string): string | null => (s.trim() ? s.trim() : null);

/** `SignOptions` de `pliego-sign` (los campos omitidos toman su valor por defecto). */
export function signOptions(f: SignForm, pageSize?: { width: number; height: number }): Record<string, unknown> {
  const o: Record<string, unknown> = { level: f.level };
  if (needsTimestamp(f.level)) o.timestamp_url = f.tsa.trim();
  if (f.visible && pageSize) {
    o.visible = { page: f.page - 1, rect: visibleRect(pageSize, f.corner) };
    o.page = f.page - 1;
  }
  if (f.certify) o.certify = f.certify;
  const reason = text(f.reason), location = text(f.location), contact = text(f.contact);
  if (reason) o.reason = reason;
  if (location) o.location = location;
  if (contact) o.contact = contact;
  return o;
}

export interface SignerRow { name: string; email: string; certificateContains: string }

/** `SignerSpec[]` del flujo; ignora las filas sin nombre. */
export function signerSpecs(rows: SignerRow[]): { name: string; email?: string; certificate_contains?: string }[] {
  return rows
    .filter((r) => r.name.trim())
    .map((r) => ({
      name: r.name.trim(),
      ...(r.email.trim() ? { email: r.email.trim() } : {}),
      ...(r.certificateContains.trim() ? { certificate_contains: r.certificateContains.trim() } : {}),
    }));
}
