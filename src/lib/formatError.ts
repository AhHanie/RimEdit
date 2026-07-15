import { formatCommandError } from "../i18n/diagnostics";

/** Normalizes any Tauri command rejection to English UI text. Delegates to
 * `src/i18n/diagnostics.ts`'s `formatCommandError`, which prefers a structured `code`/`args`
 * diagnostic lookup over the compatibility `message` field -- see
 * `docs/i18n/issues/04-frontend-diagnostic-rendering.md`. Kept as a stable, separately named
 * export since it is already called from many feature files. */
export function formatError(e: unknown): string {
  return formatCommandError(e);
}
