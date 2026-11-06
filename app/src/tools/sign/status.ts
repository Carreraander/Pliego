/** Estado legible de cada firma (puro, con pruebas). Los tipos reflejan `pliego_sign::SignatureReport`. */

export type Chain = "trusted" | "untrusted-root" | "incomplete" | "invalid";

export interface SignatureReport {
  field: string | null;
  signer: string;
  subject: string;
  signing_time: string | null;
  reason: string | null;
  location: string | null;
  integrity: boolean;
  covers_whole_document: boolean;
  modifications_allowed: boolean;
  has_dss: boolean;
  certification: number | null;
  level: Level;
  chain: Chain;
  has_timestamp: boolean;
  document_timestamp: { valid: boolean } | null;
  problems: string[];
  /** Clave estable de cada aviso de `problems` (misma posición); puede faltar en informes antiguos. */
  problem_keys?: string[];
}
type Level = "b-b" | "b-t" | "b-lt" | "b-lta";

export type Verdict = "valid" | "unknown-issuer" | "modified" | "invalid";

/** `valid` (todo bien), `unknown-issuer` (íntegra pero la cadena no es de confianza), `modified` (cambios no permitidos
 *  tras firmar) o `invalid` (el contenido firmado no coincide). */
export function verdict(r: SignatureReport): Verdict {
  if (!r.integrity || r.chain === "invalid") return "invalid";
  if (!r.modifications_allowed) return "modified";
  if (r.chain !== "trusted" || (r.document_timestamp && !r.document_timestamp.valid)) return "unknown-issuer";
  return "valid";
}

export const verdictTone = (v: Verdict): "ok" | "warn" | "danger" => (v === "valid" ? "ok" : v === "unknown-issuer" ? "warn" : "danger");

/** Resumen del documento para el aviso: el peor veredicto manda. */
export function overall(reports: SignatureReport[]): Verdict | null {
  if (!reports.length) return null;
  const order: Verdict[] = ["invalid", "modified", "unknown-issuer", "valid"];
  const vs = reports.map(verdict);
  return order.find((v) => vs.includes(v)) ?? "valid";
}

export const levelLabel = (l: Level): string => ({ "b-b": "B-B", "b-t": "B-T", "b-lt": "B-LT", "b-lta": "B-LTA" })[l];

/** Avisos que ya muestra la línea de la cadena de confianza: no se repiten como avisos sueltos. */
const CHAIN_KEYS = new Set(["untrusted-root", "chain-incomplete", "chain-invalid"]);

/**
 * Avisos de una firma en el idioma de la app. El motor manda, junto a cada texto (en español), una clave estable;
 * `translate` busca `problem.<clave>`. Un aviso sin clave conocida (`other`, o un informe antiguo) se muestra tal cual.
 */
export function problemLines(r: SignatureReport, translate: (key: string) => string): string[] {
  const keys = r.problem_keys ?? [];
  const lines: string[] = [];
  r.problems.forEach((message, i) => {
    const key = keys[i];
    if (key && CHAIN_KEYS.has(key)) return;
    const text = key && key !== "other" ? translate(`problem.${key}`) : message;
    lines.push(text === `problem.${key}` ? message : text);
  });
  return lines;
}
