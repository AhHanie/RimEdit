// One typed renderer for every backend diagnostic/command-error wire shape (see
// `src-tauri/src/diagnostics.rs` and `src/lib/diagnostics.ts`'s TypeScript mirror). Every family
// (AppError, ParseDiagnostic, ValidationDiagnostic, SchemaLoadDiagnostic, PatchDiagnostic,
// XPathDiagnostic, ApplyDiagnostic, InheritanceDiagnostic, PatchPreviewConflictDiagnostic)
// already carries a stable `code` plus typed `args`; this module looks `code` up in the
// `diagnostics` namespace and interpolates `args` verbatim. It never assembles translated text
// out of literal field/path/xpath values, and never translates those literal values itself.
// See `docs/i18n/issues/04-frontend-diagnostic-rendering.md`.

import type { i18n as I18nInstance } from "i18next";
import type { DiagnosticArgs } from "../lib/diagnostics";
import { initI18n } from "./index";

const CODES_KEY_PREFIX = "diagnostics:codes.";

/** Matches an i18next interpolation placeholder (e.g. `{{path}}`) left unresolved in rendered
 * output. i18next's default `interpolation.skipOnVariables: true` (see `src/i18n/index.ts`, which
 * does not override it) leaves a variable's `{{name}}` markup verbatim in the output whenever the
 * `args` object passed to `t()` is missing that key -- it does not throw and does not fall back to
 * `defaultValue`. Round 2's typed-interpolation-args work only enforces "all required args
 * supplied" at compile time for literal `t("some.key", { ... })` call sites; `renderDiagnostic`
 * looks up `code` as a runtime string (see `translate`'s doc comment), so that compile-time check
 * never runs here and a diagnostic with missing `args` would otherwise leak this raw markup to the
 * user. */
const UNRESOLVED_PLACEHOLDER_PATTERN = /\{\{\s*[^{}]+?\s*\}\}/;

/** Accepts a caller-supplied instance only when it is actually usable (has working `t`/`exists`
 * methods). `useTranslation()` can hand back a non-functional stub when called outside any
 * `I18nextProvider`/`LocaleProvider` tree (e.g. a component rendered directly in a test that has
 * nothing to do with locale behavior) -- falling back to the app-wide singleton in that case keeps
 * every diagnostic renderer usable without forcing every such caller to wrap its render tree in a
 * provider it otherwise has no need for. */
function resolveI18n(i18nInstance?: I18nInstance): I18nInstance {
  if (i18nInstance && typeof i18nInstance.t === "function" && typeof i18nInstance.exists === "function") {
    return i18nInstance;
  }
  return initI18n();
}

/** Calls `i18n.t` through the "dynamic key" overload (key is a runtime `string`, not one of the
 * generated literal keys) -- required for diagnostic codes, which are backend-defined and
 * inherently open-ended (see Plan.md: "Dynamic codes from Rust are validated against a typed
 * diagnostic-code registry at the frontend boundary, with a generic fallback for unknown/future
 * codes"). `defaultValue` is only ever returned when `key` is missing from the catalog; callers
 * that already checked `i18n.exists(key)` pass the key itself as a harmless placeholder. */
function translate(
  i18nInstance: I18nInstance,
  key: string,
  defaultValue: string,
  options?: Record<string, unknown>,
): string {
  return String(i18nInstance.t(key, defaultValue, options));
}

/**
 * Common shape shared by every backend diagnostic/command-error family: a stable `code`, typed
 * literal `args`, and (until issue 09 fully migrates every producer) a compatibility English
 * `message`. `code`/`args` may be absent on a wire shape issue 03 hasn't reached yet -- see
 * `renderDiagnostic`'s explicit, removable fallback for that case.
 */
export interface DiagnosticRefLike {
  code?: string | null;
  args?: DiagnosticArgs;
  message?: string | null;
}

/**
 * Renders one diagnostic/command-error reference to English UI text. Deterministic priority:
 *
 * 1. `code` present, translated in `diagnostics.codes`, and every interpolation placeholder the
 *    catalog string references was actually resolved from `args` -> the looked-up text, with
 *    `args` interpolated verbatim (field names/paths/xpath/counts are literal data, never
 *    translated). A catalog hit with one or more *unresolved* placeholders (missing/incomplete
 *    `args`) is treated as if the code were not usable at all and falls through to step 2 --
 *    see `UNRESOLVED_PLACEHOLDER_PATTERN`'s doc comment for why this can only be checked at
 *    runtime here, and Plan.md's "English fallbacks are deterministic for ... missing app keys"
 *    acceptance criterion.
 * 2. `code` missing from the catalog (most backend codes today -- issue 09 keeps migrating
 *    producers; this issue only seeds a representative set per family), or matched but left an
 *    unresolved placeholder, but a compatibility `message` is present -> `message` verbatim.
 *    Explicit and removable: this is exactly the "old `message` fallback ... during issue 03's
 *    migration window" this issue calls for, not a permanent behavior -- it disappears code by
 *    code as issue 09 adds catalog entries, and entirely once `message` itself is retired.
 * 3. `code` present, either not in the catalog or missing required args, and no `message` -> a
 *    safe generic fallback that names the stable code, so an unrecognized/malformed code stays
 *    diagnosable/reportable without ever showing raw/undefined text or a leaked `{{arg}}`
 *    placeholder.
 * 4. Neither `code` nor `message` -> a generic English fallback.
 */
export function renderDiagnostic(diag: DiagnosticRefLike, i18nInstance?: I18nInstance): string {
  const i18n = resolveI18n(i18nInstance);
  const { code, args, message } = diag;

  if (code) {
    const key = `${CODES_KEY_PREFIX}${code}`;
    if (i18n.exists(key)) {
      const rendered = translate(i18n, key, code, { ...args });
      if (!UNRESOLVED_PLACEHOLDER_PATTERN.test(rendered)) {
        return rendered;
      }
      // Missing/incomplete `args` for a real catalog key -- fall through to the same safe chain
      // used for an untranslated code rather than leak raw `{{arg}}` markup to the user.
    }
  }

  if (typeof message === "string" && message.length > 0) {
    return message;
  }

  if (code) {
    return translate(i18n, "diagnostics:unknownCode", `An unexpected error occurred (code: ${code}).`, {
      code,
    });
  }

  return translate(i18n, "diagnostics:genericError", "An unexpected error occurred.");
}

/** Best-effort literal detail for a future copy-to-clipboard/support view: the stable code (if
 * any) and the raw compatibility `message` (if any), joined verbatim and never translated. `null`
 * when there is nothing beyond what `renderDiagnostic` already shows. */
export function getDiagnosticTechnicalDetail(diag: DiagnosticRefLike): string | null {
  const parts: string[] = [];
  if (diag.code) parts.push(diag.code);
  if (typeof diag.message === "string" && diag.message.length > 0) parts.push(diag.message);
  return parts.length > 0 ? parts.join(": ") : null;
}

/** Unwraps a JSON-encoded string rejection into the object it encodes. Some Tauri command
 * rejections still surface as a raw JSON-encoded string rather than a plain `{ code, message,
 * args }` object (e.g. a command that returns `Result<T, String>`, where Tauri serializes the
 * `String` error as JSON text rather than as a structured value) -- without this unwrap, that
 * string form would fail the `typeof e === "object"` check below and fall straight to the
 * untranslated `String(e)` fallback, leaking raw JSON (or, if a caller hand-extracted `.message`
 * itself instead, raw backend English) to the user. Returns `e` unchanged when it isn't a string,
 * isn't valid JSON, or doesn't decode to an object -- only an object shape can carry
 * `code`/`args`/`message`. */
function normalizeStructuredError(e: unknown): unknown {
  if (typeof e !== "string") return e;
  try {
    const parsed: unknown = JSON.parse(e);
    return parsed !== null && typeof parsed === "object" ? parsed : e;
  } catch {
    return e;
  }
}

/** Normalizes any Tauri command rejection (a structured `AppError`, a plain `Error`, a
 * JSON-encoded string rejection -- see `normalizeStructuredError` -- or an arbitrary thrown
 * value) to English UI text through the same renderer as every other diagnostic. Backs
 * `src/lib/formatError.ts`. This is the single shared entry point: callers should never
 * special-case a JSON-string rejection locally (see `CreateDefWizard.tsx`'s `formatWizardError`,
 * which now delegates straight here instead of duplicating this unwrap). */
export function formatCommandError(e: unknown, i18nInstance?: I18nInstance): string {
  const normalized = normalizeStructuredError(e);
  if (normalized !== null && typeof normalized === "object") {
    const obj = normalized as Record<string, unknown>;
    if (typeof obj.code === "string" || typeof obj.message === "string") {
      return renderDiagnostic(
        {
          code: typeof obj.code === "string" ? obj.code : null,
          args: (obj.args as DiagnosticArgs | undefined) ?? undefined,
          message: typeof obj.message === "string" ? obj.message : null,
        },
        i18nInstance,
      );
    }
  }
  return String(e);
}

export type DiagnosticSourceKind = "parse" | "validation";

/** Translated "Parse"/"Validation" source badge text for `XmlDiagnosticsPanel`. */
export function renderDiagnosticSource(source: DiagnosticSourceKind, i18nInstance?: I18nInstance): string {
  const i18n = resolveI18n(i18nInstance);
  const fallback = source === "parse" ? "Parse" : "Validation";
  return translate(i18n, `diagnostics:source.${source}`, fallback);
}

/** Translated severity label, matching `common.severity`'s existing English values. Accepts any
 * casing (`"Error"`, `"error"`, ...) since diagnostic families disagree on casing on the wire. */
export function renderDiagnosticSeverity(severity: string, i18nInstance?: I18nInstance): string {
  const i18n = resolveI18n(i18nInstance);
  const key = `common:severity.${severity.toLowerCase()}`;
  return translate(i18n, key, severity);
}

/** Translated "Diagnostics (N)" section heading, e.g. for `PatchPreviewDialog`'s diagnostics list. */
export function renderDiagnosticSectionHeading(count: number, i18nInstance?: I18nInstance): string {
  const i18n = resolveI18n(i18nInstance);
  return translate(i18n, "diagnostics:panel.sectionHeading", `Diagnostics (${count})`, { count });
}

/** Translated "line N" / "line N:M" location wording, or `null` when there is no line to show. */
export function renderDiagnosticLocation(
  location: { line: number | null; column?: number | null },
  i18nInstance?: I18nInstance,
): string | null {
  if (location.line == null) return null;
  const i18n = resolveI18n(i18nInstance);
  if (location.column != null) {
    return translate(i18n, "diagnostics:location.lineAndColumn", `line ${location.line}:${location.column}`, {
      line: location.line,
      column: location.column,
    });
  }
  return translate(i18n, "diagnostics:location.lineOnly", `line ${location.line}`, { line: location.line });
}

export interface DiagnosticCounts {
  total: number;
  errorCount: number;
  warningCount: number;
}

/** Translated, pluralized "N issues, M errors, K warnings" summary for a diagnostics list header. */
export function renderDiagnosticCountSummary(
  counts: DiagnosticCounts,
  i18nInstance?: I18nInstance,
): string {
  const i18n = resolveI18n(i18nInstance);
  const segments = [
    translate(i18n, "diagnostics:issueCount", `${counts.total} issue(s)`, { count: counts.total }),
  ];
  if (counts.errorCount > 0) {
    segments.push(
      translate(i18n, "diagnostics:errorCount", `${counts.errorCount} error(s)`, {
        count: counts.errorCount,
      }),
    );
  }
  if (counts.warningCount > 0) {
    segments.push(
      translate(i18n, "diagnostics:warningCount", `${counts.warningCount} warning(s)`, {
        count: counts.warningCount,
      }),
    );
  }
  return segments.join(", ");
}
