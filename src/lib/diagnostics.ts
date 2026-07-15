/**
 * TypeScript mirror of `src-tauri/src/diagnostics.rs`'s shared wire shape. Every backend
 * diagnostic family (AppError, ParseDiagnostic, ValidationDiagnostic, SchemaLoadDiagnostic,
 * PatchDiagnostic, XPathDiagnostic, ApplyDiagnostic, InheritanceDiagnostic,
 * PatchPreviewConflictDiagnostic) keeps its existing `code`/`message` fields and gains a sibling
 * `args` field of this shape, omitted from the wire entirely when empty.
 *
 * `args` is typed, literal interpolation data (field names, def types/names, paths, xpath
 * strings, counts) for a future frontend renderer (see `docs/i18n/issues/04-frontend-diagnostic-rendering.md`)
 * to interpolate into a localized message looked up by `code`. It is never itself an assembled
 * sentence fragment, and this type does not participate in rendering yet -- see
 * `docs/i18n/issues/03-structured-backend-diagnostics.md`.
 */
export type DiagnosticArgValue = string | number | boolean | string[];

export type DiagnosticArgs = Record<string, DiagnosticArgValue>;

/**
 * `Error` subclass for genuinely frontend-raised failure conditions -- ones this codebase's own
 * code detects and enumerates (e.g. "no active project"), not an arbitrary/unexpected exception.
 * Carries the same stable `code`/`args` shape as a backend `AppError` so it renders through
 * `renderDiagnostic`/`formatCommandError` exactly like any backend-originated diagnostic, rather
 * than falling through to a raw, untranslatable `Error.message`. `message` is still set (to a
 * plain English sentence) so `instanceof Error` consumers and `.rejects.toThrow(...)` assertions
 * keep working, and so it still reads sensibly as a last-resort fallback if `code` is ever absent
 * from the catalog. See `docs/i18n/issues/04-frontend-diagnostic-rendering.md`.
 */
export class DiagnosticError extends Error {
  readonly code: string;
  readonly args?: DiagnosticArgs;

  constructor(code: string, message: string, args?: DiagnosticArgs) {
    super(message);
    this.name = "DiagnosticError";
    this.code = code;
    this.args = args;
  }
}
