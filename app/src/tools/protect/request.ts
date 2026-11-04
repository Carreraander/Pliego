/** Construcción y validación de las peticiones de Proteger y Sanear (puro, sin interfaz: lo prueban los tests). */

export const PERMISSIONS = ["print", "copy", "modify", "annotate", "fill_forms", "assemble", "accessibility", "print_high_quality"] as const;
export type PermissionKey = (typeof PERMISSIONS)[number];
export type Permissions = Record<PermissionKey, boolean>;
export const ALL_ALLOWED: Permissions = {
  print: true, copy: true, modify: true, annotate: true, fill_forms: true, assemble: true, accessibility: true, print_high_quality: true,
};

export interface ProtectForm {
  userPassword: string;
  repeat: string;
  ownerPassword: string;
  /** Contraseña con la que se abrió el documento, si ya estaba cifrado. */
  current: string;
  cipher: "Aes256" | "Aes128";
  permissions: Permissions;
}

export interface ProtectRequest {
  path: string;
  password: string | null;
  userPassword: string;
  ownerPassword: string;
  cipher: "Aes256" | "Aes128";
  permissions: Permissions;
}

export type Built<T> = { ok: true; request: T } | { ok: false; error: string };

const utf8Len = (s: string): number => new TextEncoder().encode(s).length;

/** Valida el formulario con las mismas reglas que `pliego-protect` para avisar antes de lanzar el trabajo. */
export function buildProtectRequest(path: string, f: ProtectForm): Built<ProtectRequest> {
  if (!f.userPassword && !f.ownerPassword) return { ok: false, error: "protect.err.none" };
  if (f.userPassword && f.userPassword !== f.repeat) return { ok: false, error: "protect.err.mismatch" };
  const restricted = PERMISSIONS.some((k) => !f.permissions[k]);
  if (restricted && !f.ownerPassword) return { ok: false, error: "protect.err.owner" };
  if (utf8Len(f.userPassword) > 127 || utf8Len(f.ownerPassword) > 127) return { ok: false, error: "protect.err.long" };
  return {
    ok: true,
    request: { path, password: f.current || null, userPassword: f.userPassword, ownerPassword: f.ownerPassword, cipher: f.cipher, permissions: { ...f.permissions } },
  };
}

export const SANITIZE_OPTIONS = [
  "metadata", "javascript", "open_actions", "attachments", "embedded_files", "comments", "form_data", "thumbnails", "hidden_layers",
] as const;
export type SanitizeKey = (typeof SANITIZE_OPTIONS)[number];
export type SanitizeOptions = Record<SanitizeKey, boolean>;
export const SANITIZE_ALL: SanitizeOptions = {
  metadata: true, javascript: true, open_actions: true, attachments: true, embedded_files: true, comments: true, form_data: true, thumbnails: true, hidden_layers: true,
};

export interface SanitizeRequest { path: string; password: string | null; options: SanitizeOptions }

export function buildSanitizeRequest(path: string, current: string, options: SanitizeOptions): Built<SanitizeRequest> {
  if (!SANITIZE_OPTIONS.some((k) => options[k])) return { ok: false, error: "sanitize.err.none" };
  return { ok: true, request: { path, password: current || null, options: { ...options } } };
}

/** Nombre de archivo para el título de la tarea. */
export const baseName = (path: string): string => path.split(/[\\/]/).pop() ?? path;

// ── Redactar ────────────────────────────────────────────────────────────────────────

export const REDACT_PATTERNS = ["dni", "nie", "cif", "iban", "email", "telefono", "fecha"] as const;
export type RedactPattern = (typeof REDACT_PATTERNS)[number];

export interface RedactForm {
  patterns: RedactPattern[];
  /** Expresión regular propia; vacía = no se usa. */
  regex: string;
  /** Si la casilla de la expresión propia está marcada. */
  useRegex: boolean;
  areas: { page: number; x0: number; y0: number; x1: number; y1: number }[];
  fill: "Black" | "None";
  images: "Pixels" | "Remove" | "Keep";
  removeLineArt: boolean;
  current: string;
}

export interface RedactRequest {
  path: string;
  password: string | null;
  areas: RedactForm["areas"];
  patterns: string[];
  options: { fill: RedactForm["fill"]; images: RedactForm["images"]; remove_line_art: boolean };
}

/** Los patrones que se pasan al motor (`dni`, … o `regex:<expresión>`). */
export function patternNames(f: Pick<RedactForm, "patterns" | "useRegex" | "regex">): string[] {
  return [...f.patterns, ...(f.useRegex && f.regex.trim() ? [`regex:${f.regex.trim()}`] : [])];
}

export function buildRedactRequest(path: string, f: RedactForm): Built<RedactRequest> {
  if (f.useRegex && !f.regex.trim()) return { ok: false, error: "redact.err.regex" };
  const patterns = patternNames(f);
  if (patterns.length === 0 && f.areas.length === 0) return { ok: false, error: "redact.err.nothing" };
  return {
    ok: true,
    request: {
      path, password: f.current || null, areas: f.areas.map((a) => ({ ...a })), patterns,
      options: { fill: f.fill, images: f.images, remove_line_art: f.removeLineArt },
    },
  };
}
