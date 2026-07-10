export function formatError(e: unknown): string {
  if (e !== null && typeof e === "object") {
    const obj = e as Record<string, unknown>;
    if (typeof obj.message === "string") return obj.message;
  }
  return String(e);
}
