/** Mensaje de un error de comando (`ApiError { code, message }`) o de cualquier excepción. */
export function errorMessage(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) return String((e as { message: unknown }).message);
  return String(e);
}
