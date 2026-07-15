// Shared error-construction helper for `useProjectFiles.ts`. The "no active project" precondition
// guarding every mutating call in that hook (create/rename/delete a file or folder) is a known,
// enumerable condition the hook detects itself -- not an arbitrary/unexpected failure -- so it
// must carry a structured `code` (looked up in `diagnostics:codes.project_file_no_active_project`)
// rather than a plain `Error` whose English `message` would bypass translation entirely (see
// `src/i18n/diagnostics.ts`'s `renderDiagnostic`: the raw-`message` fallback is meant only for
// genuinely unstructured/unexpected errors, not a condition this codebase's own code enumerates).
// Mirrors `src/features/form-views/lib/formViewErrors.ts`'s `noActiveProjectError` for the same
// bug class in a different feature.
import { DiagnosticError } from "../../../lib/diagnostics";

/** Builds the shared "no active project" diagnostic error for project-file mutations. `message`
 * is kept as a plain English sentence, both as the offline/last-resort fallback `renderDiagnostic`
 * still honors for an untranslated locale, and so `instanceof Error`/`.rejects.toThrow(...)`
 * consumers keep working unchanged -- only the translated catalog text (looked up via `code`) is
 * shown once a locale/catalog entry exists. */
export function noActiveProjectError(message: string): DiagnosticError {
  return new DiagnosticError("project_file_no_active_project", message);
}
