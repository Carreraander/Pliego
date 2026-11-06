/** Informes de las comprobaciones (W29) y su forma común para mostrarlos en una lista. */
import { engineText } from "../../i18n/engine";

export interface A11yReport {
  pages: number; tagged: boolean; language: string | null; title: string | null;
  issues: { code?: string; severity: "error" | "warning"; page: number | null; xref?: number | null; message: string; suggestion: string }[];
}
export interface PdfaValidation { level: string; issues: { page: number | null; message: string }[] }
export interface PreflightReport {
  profile: string; pages: number;
  findings: { severity: "info" | "warning" | "error"; page: number | null; message: string }[];
}

export type Tone = "danger" | "warn" | "info";
export interface Row { tone: Tone; page: number | null; text: string; hint?: string }
export interface Summary { errors: number; warnings: number; rows: Row[] }

const sum = (rows: Row[]): Summary => ({
  errors: rows.filter((r) => r.tone === "danger").length,
  warnings: rows.filter((r) => r.tone === "warn").length,
  rows,
});

export function fromA11y(r: A11yReport): Summary {
  return sum(r.issues.map((i) => ({ tone: i.severity === "error" ? "danger" : "warn", page: i.page, text: engineText(i.message), hint: engineText(i.suggestion) })));
}
/** Todo incumplimiento de PDF/A cuenta como error. */
export function fromPdfa(v: PdfaValidation): Summary {
  return sum(v.issues.map((i) => ({ tone: "danger", page: i.page, text: engineText(i.message) })));
}
export function fromPreflight(r: PreflightReport): Summary {
  const tone = { error: "danger", warning: "warn", info: "info" } as const;
  return sum(r.findings.map((f) => ({ tone: tone[f.severity], page: f.page, text: engineText(f.message) })));
}
