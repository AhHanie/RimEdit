// Shared error-construction helpers for `useXmlEditorSession.ts`. Each of these preconditions
// (document read-only or no active file, no Def selected, no active project) is a known,
// enumerable condition the hook detects itself -- not an arbitrary/unexpected failure -- so each
// must carry a structured `code` (looked up in `diagnostics:codes.*`) rather than a plain `Error`
// whose English `message` would bypass translation entirely (see `src/i18n/diagnostics.ts`'s
// `renderDiagnostic`: the raw-`message` fallback is meant only for genuinely unstructured/
// unexpected errors, not a condition this codebase's own code enumerates). Mirrors
// `src/features/form-views/lib/formViewErrors.ts`'s `noActiveProjectError` for the same bug class
// in a different feature.
import { DiagnosticError } from "../../../lib/diagnostics";

/** Builds the shared "document is read-only or no file is open" diagnostic error, raised by
 * `insertDefFromTemplate`/`insertDefFromUserTemplate`/`insertDefFromIndexedDef`/
 * `saveSelectedDefAsTemplate` when `readOnly || !projectId || !relativePath`. `message` is kept
 * as a plain English sentence describing the specific action that was attempted, both as the
 * offline/last-resort fallback `renderDiagnostic` still honors for an untranslated locale, and so
 * `instanceof Error`/`.rejects.toThrow(...)` consumers keep working unchanged -- only the
 * translated catalog text (looked up via `code`) is shown once a locale/catalog entry exists. */
export function noActiveFileError(message: string): DiagnosticError {
  return new DiagnosticError("xml_editor_session_no_active_file", message);
}

/** Builds the "no Def is selected" diagnostic error, raised by `saveSelectedDefAsTemplate` when
 * there is no `selectedDefNodeId`. */
export function noDefSelectedError(message: string): DiagnosticError {
  return new DiagnosticError("xml_editor_session_no_def_selected", message);
}

/** Builds the "no active project" diagnostic error, raised by `deleteUserDefTemplate` when there
 * is no `projectId`. */
export function noActiveProjectError(message: string): DiagnosticError {
  return new DiagnosticError("xml_editor_session_no_active_project", message);
}

/** Builds the "form edit returned no parsed document" diagnostic error, raised by
 * `applyFormEdits` when the backend's `document` field comes back `null`. This is not a
 * can't-happen invariant: `apply_editor_edits` in `src-tauri/src/services/xml_editor.rs`
 * intentionally re-parses the caller-supplied `raw_xml` before applying any edits and, if that
 * already has a fatal parse error, returns early with `document: null` without attempting the
 * edit. That is genuinely reachable through normal use -- a form field's debounced commit can
 * still be in flight (or queued behind `pendingFormEditRef`) when the user switches to the raw
 * XML tab and types something unparseable; `latestRawXmlRef.current` updates immediately (see
 * `updateRawXml`), so the queued commit's `currentXml` can already be fatally malformed by the
 * time it runs. This exact path used to throw a raw `Error` with no `code`, which
 * `formatCommandError` renders verbatim instead of translating. */
export function formEditNoDocumentError(message: string): DiagnosticError {
  return new DiagnosticError("xml_editor_session_form_edit_no_document", message);
}
