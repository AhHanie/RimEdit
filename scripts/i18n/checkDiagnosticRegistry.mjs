// Shared pure logic for scripts/i18n/check-diagnostic-registry.mjs. Cross-references the frontend
// diagnostic-code catalog (src/i18n/resources/en/diagnostics.json's "codes" object, rendered by
// src/i18n/diagnostics.ts) against every source tree that can originate a `code` string, so a
// renamed/removed diagnostic code cannot silently leave a stale, unreachable entry behind in the
// English catalog (docs/i18n/issues/10-formatting-rtl-and-release-tooling.md step 4:
// "diagnostic-registry checks"). Complements scripts/i18n/check-diagnostic-args.mjs, which checks
// the Rust *struct shape* (does a diagnostic family have an `args` field at all); this checks the
// frontend *catalog contents* against the *code strings* that actually exist anywhere they can be
// constructed.
//
// Codes are not exclusively backend-owned: most cross the wire from Rust, but a handful of
// diagnostics are computed client-side as companions to a backend classification (e.g.
// `src/features/patches-editor/lib/operationClassificationDiagnostics.ts`'s
// `patch_custom_operation_unexecutable`/`patch_operation_class_unrecognized`, which mirror
// `patches::index::PatchOperationClassification` at the single-file-editing level before a Def
// preview round-trip is possible) -- so this scans both `src-tauri/src` and `src`, not Rust alone.
//
// The orphan-catalog-entry direction (findOrphanedCatalogCodes) is deliberately a coarse "does
// this literal string appear anywhere in either tree" check, not a full data-flow analysis -- see
// findCodeLikeStringLiterals's doc comment. It fails the build for a catalog key that appears
// nowhere at all, which is unambiguously stale/wrong.
//
// The reverse direction (findProducedDiagnosticCodes/findMissingCatalogCodes, below) fails the
// build for a code actually constructed in src-tauri/src with no catalog entry at all -- a
// recurring bug class that used to be found and fixed piecemeal, one instance at a time. It
// intentionally does NOT reuse the coarse "any code-shaped literal" scan (that would flag dozens
// of unrelated snake_case literals as missing codes -- see that section's own doc comment for
// why), so it is a narrower, more precise scan than the orphan direction.

/** A diagnostic/error `code` string is always either flat `snake_case` (the documented convention
 * in docs/i18n/diagnostic-codes.md, e.g. `validation_missing_required_field`) or, for a small
 * number of pre-existing codes, `SCREAMING_SNAKE_CASE` (e.g. `TOKEN_NOT_FOUND`). Requires at least
 * one underscore-separated segment so single-word literals (most non-code strings) don't match. */
const CODE_LIKE_PATTERN = /^([a-z][a-z0-9]*(?:_[a-z0-9]+)+|[A-Z][A-Z0-9]*(?:_[A-Z0-9]+)+)$/;

/** True when `text` has the lexical shape of a diagnostic code (see `CODE_LIKE_PATTERN`). Exported
 * so this checker's own coverage of "what counts as code-shaped" is directly testable. */
export function looksLikeDiagnosticCode(text) {
  return CODE_LIKE_PATTERN.test(text);
}

// `\\[\s\S]` (not `\\.`) so a backslash-newline line-continuation inside a multi-line Rust string
// literal -- e.g. `"...text; \` followed by a wrapped continuation line, used throughout this
// codebase's longer `#[error(...)]`/diagnostic messages -- is consumed as part of the escape
// rather than desynchronizing the rest of the file's quote pairing (`.` never matches a line
// terminator in a JS regex, so `\\.` alone silently breaks on exactly this pattern).
const DOUBLE_QUOTED_STRING = /"((?:[^"\\]|\\[\s\S])*)"/g;

/** Scans raw source text (Rust or TypeScript -- both use `"..."` string literals per this repo's
 * `cargo fmt`/CLAUDE.md double-quote conventions) for every double-quoted string literal that
 * looks like a diagnostic code (`looksLikeDiagnosticCode`). This intentionally over-matches --
 * most snake_case string literals in either tree are not diagnostic codes at all (XML tag names,
 * JSON field names, struct field literals in tests, ...) -- because the only thing this checker
 * ever fails the build on is a catalog key that matches NONE of these, which stays correct
 * regardless of how many unrelated code-shaped strings also get collected. */
export function findCodeLikeStringLiterals(sourceText) {
  const found = new Set();
  for (const match of sourceText.matchAll(DOUBLE_QUOTED_STRING)) {
    const text = match[1];
    if (looksLikeDiagnosticCode(text)) {
      found.add(text);
    }
  }
  return found;
}

/**
 * Returns the subset of `catalogCodes` that appears in none of `codeLikeStringSets` -- i.e.
 * frontend `diagnostics:codes.*` keys with no corresponding literal anywhere in the scanned trees.
 */
export function findOrphanedCatalogCodes(catalogCodes, codeLikeStringSets) {
  const allCodes = new Set();
  for (const set of codeLikeStringSets) {
    for (const code of set) allCodes.add(code);
  }
  return catalogCodes.filter((code) => !allCodes.has(code));
}

// ---------------------------------------------------------------------------
// Reverse direction: a code actually constructed with no catalog entry.
//
// The orphan check above is deliberately coarse (any code-shaped literal anywhere counts as
// "exists"). The reverse direction cannot reuse that same coarseness: naively treating every
// code-shaped string literal in src-tauri/src as "produced" flags dozens of unrelated snake_case
// literals (test fixture names, `#[serde(default = "...")]` function-name references, path
// segments passed to unrelated helpers, negative-test code strings that assert a code is NEVER
// produced) that are not diagnostic codes at all -- which would make this check permanently noisy
// and untrustworthy. Instead this scans only the specific, closed set of syntactic shapes this
// codebase's diagnostic families actually use to attach a `code` (see
// docs/i18n/diagnostic-codes.md and src-tauri/src/diagnostics.rs's module docs), which keeps the
// false-positive rate low enough to fail the build on.
// ---------------------------------------------------------------------------

/** The closed set of diagnostic-family type names this codebase's `code`/`args` wire contract
 * uses -- copied from src-tauri/src/diagnostics.rs's own module doc comment (which enumerates
 * this exact list) plus `DiagnosticRef` itself. Deliberately closed/explicit for `::error(`/
 * `::warning(`/`::code(` -- those method names ARE too common on unrelated types to use as a
 * generic marker. `::new(` gets one narrow, deliberately-scoped exception instead of joining this
 * closed list: see `CONSTRUCTOR_CALL_START_PATTERN`'s doc comment for the `Warning`/`Diagnostic`/
 * `Error`-suffix generalization that also covers types outside this list (`GraphicPreviewWarning`,
 * ...) without a large false-positive surface (a bare `\bAnything::new\(` would still be rejected --
 * `new` alone is far too common on unrelated Rust types: `Vec::new()`, `PathBuf::new()`, ...).
 * `LoadFolderDiagnostic` (src-tauri/src/rimworld_load_folders.rs, itself struct-literal
 * constructed, not `::new(`-constructed, so the suffix exception above is moot for it anyway) is
 * deliberately NOT included in this list: its `code`/`message` fields are `#[allow(dead_code)]`,
 * it is never `Serialize`d, and it never crosses the Tauri IPC boundary -- "reserved for future
 * surfacing" per its own doc comment, not a currently-produced user-facing diagnostic.
 * `findProducedDiagnosticCodes` excludes its whole file by path (see this function's `filePath`
 * param doc comment) so its `code: "..."` struct-literal fields don't false-positive against
 * `CODE_FIELD_PATTERN` either. */
const DIAGNOSTIC_FAMILY_TYPES = [
  "AppError",
  "ParseDiagnostic",
  "ValidationDiagnostic",
  "SchemaLoadDiagnostic",
  "PatchDiagnostic",
  "XPathDiagnostic",
  "ApplyDiagnostic",
  "InheritanceDiagnostic",
  "PatchPreviewConflictDiagnostic",
  "DiagnosticRef",
];

/** Matches the start (through the opening `(`) of a call this codebase uses to attach a `code` to
 * a diagnostic: a family constructor (`ValidationDiagnostic::error(`, `XPathDiagnostic::warning(`,
 * `DiagnosticRef::code(`, ...), the `.with_code(` builder method, one of the small number of
 * shared positional helper functions (`error_at`/`warning_at`/`error_at_node`/`warning_at_node`,
 * see src-tauri/src/xml_document/validation/{diagnostics,about}.rs) that wrap a family constructor
 * with `code`/`message` as plain parameters, or a `::new(` call on any OTHER type whose name ends
 * in `Warning`/`Diagnostic`/`Error` -- this codebase's established naming convention for every
 * `code`/`message`/`args`-shaped diagnostic type, closed-list `DIAGNOSTIC_FAMILY_TYPES` included
 * (e.g. `GraphicPreviewWarning::new(`, `FormViewStoreWarning::new(` if it ever grows one). Unlike a
 * bare `\bAnything::new\(` (rejected -- see `DIAGNOSTIC_FAMILY_TYPES`'s doc comment on why `new` is
 * too common a generic marker: `Vec::new()`, `PathBuf::new()`, ...), this suffix-scoped variant
 * stays narrow: nothing in this codebase defines a `Warning`/`Diagnostic`/`Error`-suffixed type
 * with a `::new(...)` whose arguments could plausibly contain an unrelated code-shaped literal
 * (verified against every existing call site in this codebase), and it needs no further
 * manual update when a future warning/diagnostic type is added following the same convention. */
const CONSTRUCTOR_CALL_START_PATTERN = new RegExp(
  String.raw`\b(?:${DIAGNOSTIC_FAMILY_TYPES.join("|")})::(?:new|error|warning|code)\(` +
    String.raw`|\b[A-Za-z_][A-Za-z0-9_]*(?:Warning|Diagnostic|Error)::new\(` +
    String.raw`|\.with_code\(` +
    String.raw`|\b(?:error_at_node|warning_at_node|error_at|warning_at)\(`,
  "g",
);

/** Matches a `code` struct-literal field assigned a string literal directly, e.g.
 * `code: "io_error".to_string()` or `code: "watcher_setup_failed".into()` -- the shape
 * `AppError`/`LoadFolderDiagnostic`-style plain structs use instead of a family constructor. */
const CODE_FIELD_PATTERN = /\bcode:\s*"([A-Za-z][A-Za-z0-9_]*)"/g;

/** Rust source text with every `#[cfg(test)]` module body removed (brace-balanced, not just the
 * next line) so an inline test module mixed into a production file -- e.g.
 * src-tauri/src/diagnostics.rs's own `mod tests`, which constructs a diagnostic with a
 * deliberately fake code like `"some_code"` purely to exercise wire-shape serialization -- cannot
 * contribute a fake "produced" code. Dedicated test files are filtered separately by path (see
 * `isRustTestPath`); this handles the common same-file `#[cfg(test)] mod ... { ... }` pattern this
 * codebase uses throughout (e.g. `src-tauri/src/patches/apply.rs`'s `diagnostic_ref_wire_tests`).
 * A missing/malformed brace pair degrades to "strip nothing from here on", which only risks a
 * missed real code (safe direction), never a false failure. */
export function stripCfgTestModules(sourceText) {
  const marker = "#[cfg(test)]";
  let result = "";
  let i = 0;
  for (;;) {
    const markerIndex = sourceText.indexOf(marker, i);
    if (markerIndex === -1) {
      result += sourceText.slice(i);
      return result;
    }
    result += sourceText.slice(i, markerIndex);
    const braceStart = sourceText.indexOf("{", markerIndex);
    if (braceStart === -1) {
      return result; // No opening brace found after the marker -- stop scanning defensively.
    }
    let depth = 1;
    let j = braceStart + 1;
    while (j < sourceText.length && depth > 0) {
      if (sourceText[j] === "{") depth++;
      else if (sourceText[j] === "}") depth--;
      j++;
    }
    i = j;
  }
}

/** True for a Rust source path that holds only test code -- a dedicated `tests/` directory or a
 * `*_test.rs` file. Excluded from `findProducedDiagnosticCodes` for the same reason
 * `stripCfgTestModules` strips inline `#[cfg(test)]` modules: test code exercises diagnostic
 * constructors with intentionally fake/example codes that were never meant to reach a real user or
 * need a catalog entry. */
export function isRustTestPath(filePath) {
  const normalized = filePath.replace(/\\/g, "/");
  return /(?:^|\/)tests\//.test(normalized) || /_test\.rs$/.test(normalized);
}

/** Extracts the substring between a call's opening `(` (at `openParenIndex`, pointing at the `(`
 * itself) and its balanced closing `)`. Brace-matching only counts parens, not string contents, so
 * a `)` inside a string literal argument (rare in this codebase's diagnostic messages) can close
 * the scan early -- an accepted approximation, since it only risks missing a code that appears
 * later in the same call (safe direction) rather than misattributing one from a different call. */
function extractBalancedCallArgs(sourceText, openParenIndex) {
  let depth = 1;
  let i = openParenIndex + 1;
  while (i < sourceText.length && depth > 0) {
    const ch = sourceText[i];
    if (ch === "(") depth++;
    else if (ch === ")") depth--;
    i++;
  }
  return sourceText.slice(openParenIndex + 1, i - 1);
}

/** Matches this codebase's other common code-attaching shape: a `From<XError> for AppError`
 * conversion that maps each source-error variant to its code via `let code = match &e { ... };`
 * (e.g. `src-tauri/src/project_files/error.rs`, `def_templates/error.rs`, `form_views/error.rs`,
 * `def_index/cache.rs`, `patches/cache.rs`, `project_save.rs`, `project_model.rs`'s `StoreError`
 * conversion) -- a plain enum-variant match, not a family constructor call, so it needs its own
 * scan rather than `CONSTRUCTOR_CALL_START_PATTERN`. */
const CODE_MATCH_BLOCK_START_PATTERN = /\blet\s+code\s*=\s*match\b/g;

/** Every code-shaped string literal on the right-hand side of a `=>` match arm within
 * `blockText` (the body of a `let code = match ... { ... }` block extracted via
 * `extractBalancedBraces`) -- covers both `Variant(_) => "code",` and multi-pattern arms like
 * `A | B => "code",`. */
function findMatchArmCodes(blockText) {
  const codes = [];
  for (const match of blockText.matchAll(/=>\s*"([A-Za-z][A-Za-z0-9_]*)"/g)) {
    if (looksLikeDiagnosticCode(match[1])) codes.push(match[1]);
  }
  return codes;
}

/** Extracts the substring between a block's opening `{` (found by scanning forward from
 * `fromIndex`) and its balanced closing `}`. Returns `""` if no opening brace is found. Shares
 * `stripCfgTestModules`'s brace-counting approach (parens/strings not accounted for). */
function extractBalancedBraces(sourceText, fromIndex) {
  const braceStart = sourceText.indexOf("{", fromIndex);
  if (braceStart === -1) return "";
  let depth = 1;
  let i = braceStart + 1;
  while (i < sourceText.length && depth > 0) {
    if (sourceText[i] === "{") depth++;
    else if (sourceText[i] === "}") depth--;
    i++;
  }
  return sourceText.slice(braceStart + 1, i - 1);
}

/**
 * Scans Rust source text for diagnostic codes actually constructed via one of this codebase's
 * known code-attaching shapes (see `CONSTRUCTOR_CALL_START_PATTERN`/`CODE_FIELD_PATTERN`'s doc
 * comments), after stripping inline `#[cfg(test)]` modules. For each matched constructor call, the
 * code is taken as the first code-shaped (`looksLikeDiagnosticCode`) double-quoted string literal
 * within that call's balanced argument list -- code is not always positionally first (e.g.
 * `ValidationDiagnostic::error(relative_path, node_id, line, column, code, message)` has a path
 * first), but a relative path/message argument is never itself code-shaped (paths contain `.`/`/`,
 * messages contain spaces), so the first code-shaped literal in the call reliably is the code.
 *
 * `filePath` (optional) excludes `src-tauri/src/rimworld_load_folders.rs`'s `LoadFolderDiagnostic`
 * struct: its `code`/`message` fields are `#[allow(dead_code)]`, it is never `Serialize`d, and it
 * never crosses the Tauri IPC boundary ("reserved for future surfacing" per its own doc comment) --
 * so its `code: "..."` field literals would otherwise false-positive against `CODE_FIELD_PATTERN`,
 * which (unlike `CONSTRUCTOR_CALL_START_PATTERN`) has no way to tell which struct a bare `code:`
 * field belongs to.
 */
export function findProducedDiagnosticCodes(sourceText, filePath) {
  const normalizedPath = (filePath ?? "").replace(/\\/g, "/");
  const isNonWireDiagnosticFile = normalizedPath.endsWith("rimworld_load_folders.rs");
  const stripped = stripCfgTestModules(sourceText);
  const found = new Set();

  if (isNonWireDiagnosticFile) return found;

  for (const match of stripped.matchAll(CODE_FIELD_PATTERN)) {
    if (looksLikeDiagnosticCode(match[1])) found.add(match[1]);
  }

  for (const match of stripped.matchAll(CONSTRUCTOR_CALL_START_PATTERN)) {
    const openParenIndex = match.index + match[0].length - 1;
    const args = extractBalancedCallArgs(stripped, openParenIndex);
    const literalMatch = args.match(/"([A-Za-z][A-Za-z0-9_]*)"/);
    if (literalMatch && looksLikeDiagnosticCode(literalMatch[1])) {
      found.add(literalMatch[1]);
    }
  }

  for (const match of stripped.matchAll(CODE_MATCH_BLOCK_START_PATTERN)) {
    const blockText = extractBalancedBraces(stripped, match.index + match[0].length);
    for (const code of findMatchArmCodes(blockText)) found.add(code);
  }

  return found;
}

/**
 * Returns every code in `producedCodes` that has no entry in `catalogCodes` -- a diagnostic code
 * this codebase actually constructs (per `findProducedDiagnosticCodes`) with nothing in
 * `diagnostics.json`'s `codes` object to render it, other than the generic `message` fallback.
 * This is the reverse of `findOrphanedCatalogCodes`: that one catches a stale catalog entry, this
 * one catches the recurring bug class of a new/renamed code that was never added to the catalog.
 */
export function findMissingCatalogCodes(producedCodes, catalogCodes) {
  const catalogSet = new Set(catalogCodes);
  return [...producedCodes].filter((code) => !catalogSet.has(code)).sort();
}
