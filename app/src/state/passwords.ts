/**
 * Contraseñas de los documentos abiertos, solo en memoria (nunca se guardan en disco ni en los ajustes).
 *
 * Cuando una persona abre un PDF protegido y escribe la contraseña, los paneles que lanzan trabajos sobre ese mismo
 * archivo (comprimir, OCR, convertir, comparar, sellos, numerar…) la reutilizan en vez de pedirla otra vez o fallar.
 * Se olvida al cerrar el documento.
 */
const remembered = new Map<string, string>();

export function rememberPassword(path: string, password: string): void {
  if (password) remembered.set(path, password);
}

export function forgetPassword(path: string): void {
  remembered.delete(path);
}

export function passwordFor(path: string): string | null {
  return remembered.get(path) ?? null;
}

type Args = Record<string, unknown>;
const isObject = (v: unknown): v is Args => typeof v === "object" && v !== null && !Array.isArray(v);

/**
 * Añade la contraseña recordada a los argumentos de un comando que actúa sobre `path`, sin pisar una que ya venga
 * puesta. Reconoce las dos formas que usa la app: `{ path, password }` y `{ request: { path, password } }`, y
 * `{ path, passwordA }` (Comparar). Devuelve una copia; los argumentos originales no cambian.
 */
export function withPassword(args: Args): Args {
  const out: Args = { ...args };
  const fill = (target: Args, key: string, path: unknown): Args => {
    if (typeof path !== "string" || target[key] != null) return target;
    const pw = passwordFor(path);
    return pw === null ? target : { ...target, [key]: pw };
  };
  let next = fill(out, "password", out.path);
  next = fill(next, "passwordA", out.path);
  // `passwordA` solo tiene sentido en Comparar; en los demás comandos Tauri ignora los argumentos que no conoce.
  if (!("passwordA" in args) && !("other" in args)) delete next.passwordA;
  if (isObject(next.request)) {
    let req: Args = next.request;
    req = fill(req, "password", req.path);
    next = { ...next, request: req };
  }
  return next;
}
