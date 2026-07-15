// Shared error-construction helper for `useFormViews.ts`/`useCustomFormViews.ts`. The "no active
// project" precondition guarding most of this feature's mutating calls is a known, enumerable
// condition both hooks detect themselves -- not an arbitrary/unexpected failure -- so it must
// carry a structured `code` (looked up in `diagnostics:codes.form_view_no_active_project`) rather
// than a plain `Error` whose English `message` would bypass translation entirely (see
// `src/i18n/diagnostics.ts`'s `renderDiagnostic`: the raw-`message` fallback is meant only for
// genuinely unstructured/unexpected errors, not a condition this codebase's own code enumerates).
import { DiagnosticError } from "../../../lib/diagnostics";

/** Builds the shared "no active project" diagnostic error. `message` is kept as a plain English
 * sentence describing the specific action that was attempted, both as the offline/last-resort
 * fallback `renderDiagnostic` still honors for an untranslated locale, and so `instanceof Error`/
 * `.rejects.toThrow(...)` consumers keep working unchanged -- only the translated catalog text
 * (looked up via `code`) is shown once a locale/catalog entry exists. */
export function noActiveProjectError(message: string): DiagnosticError {
  return new DiagnosticError("form_view_no_active_project", message);
}

/** Builds the "no unsaved Form View changes to save" diagnostic error, raised by
 * `useFormViews.ts`'s `saveOverrideAsCustomView` when there is no dirty `FieldVisibilityOverride`
 * to persist -- a known, enumerable precondition this hook detects itself, not an arbitrary/
 * unexpected failure. `message` is kept as a plain English sentence for the same reasons as
 * `noActiveProjectError` above. */
export function noUnsavedChangesError(message: string): DiagnosticError {
  return new DiagnosticError("form_view_no_unsaved_changes", message);
}
