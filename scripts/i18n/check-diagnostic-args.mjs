#!/usr/bin/env node
// Guards against a new backend diagnostic/error wire struct being added without the shared
// code+args mechanism (`crate::diagnostics::DiagnosticArgs`, see `src-tauri/src/diagnostics.rs`
// and `docs/i18n/diagnostic-codes.md`). Per
// docs/i18n/issues/09-validation-and-diagnostic-migration.md step 6: "Add a guard/search-based CI
// check preventing new public diagnostics from accepting bare message-only constructors."
//
// A "message-only constructor" here means a `pub struct ...Diagnostic` / `pub struct ...Error`
// that carries a `code`/`message` pair but has no `args: ... DiagnosticArgs` field at all -- i.e.
// it is structurally incapable of ever carrying typed, localizable interpolation data, unlike
// every migrated family (`AppError`, `ParseDiagnostic`, `ValidationDiagnostic`,
// `SchemaLoadDiagnostic`, `PatchDiagnostic`, `XPathDiagnostic`, `ApplyDiagnostic`,
// `InheritanceDiagnostic`, `PatchPreviewConflictDiagnostic`, `DefIndexError`, `PatchIndexError`).
// This is a source-level structural check (does the struct declare an `args` field), not a
// per-call-site check of whether every individual construction populates it -- many construction
// sites legitimately omit args when the message wraps arbitrary third-party/IO text with no
// literal identifier to extract (see Plan.md: "do not make arbitrary English library text a
// translation key").
//
// Usage: node scripts/i18n/check-diagnostic-args.mjs

import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = join(fileURLToPath(import.meta.url), "..", "..", "..");
const srcRoot = join(repoRoot, "src-tauri", "src");

// Struct names that legitimately carry a `code`/`message` pair without an `args` field, with the
// reason inline. Adding a name here must be a deliberate, reviewed decision -- not a default.
const EXEMPTIONS = {
  // Never serialized to the frontend: `LoadFolderResolution.diagnostics` is explicitly
  // `#[allow(dead_code)]` ("Reserved for future surfacing"), so there is no wire contract to
  // localize yet. Revisit (remove this exemption and add `args`) once it is actually surfaced.
  LoadFolderDiagnostic: "not yet surfaced to the frontend (see rimworld_load_folders.rs)",
  // Never reaches the wire as a structured object: `patches::dom::parse_fragment` callers collapse
  // this to a plain `String` before it ever reaches an `AppError`/command result (see
  // commands/patches.rs::parse_patch_value_xml), so it has no `code` to attach args to either.
  FragmentDiagnostic: "collapsed to a plain String before crossing the Tauri boundary (see patches/dom.rs)",
};

/** Recursively lists every `.rs` file under `dir`. */
function listRustFiles(dir) {
  const out = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    const stat = statSync(full);
    if (stat.isDirectory()) {
      out.push(...listRustFiles(full));
    } else if (entry.endsWith(".rs")) {
      out.push(full);
    }
  }
  return out;
}

/**
 * Finds every `pub struct <Name>Diagnostic` / `pub struct <Name>Error` declaration and reports
 * one whose body (up to its matching closing brace) has no `args` field, and is not in
 * `EXEMPTIONS`.
 */
export function findBareMessageOnlyDiagnostics(files) {
  const violations = [];
  const structPattern = /pub struct (\w*(?:Diagnostic|Error))\s*\{/g;

  for (const file of files) {
    const text = readFileSync(file, "utf8");
    for (const match of text.matchAll(structPattern)) {
      const name = match[1];
      // `DiagnosticArgs`/`DiagnosticRef`/`DiagnosticArgValue` are the shared mechanism itself, not
      // a diagnostic payload struct.
      if (name === "DiagnosticArgs" || name === "DiagnosticRef" || name === "DiagnosticArgValue") {
        continue;
      }
      const bodyStart = match.index + match[0].length;
      const body = extractBalancedBraceBody(text, bodyStart);
      if (body === null) continue; // malformed/unbalanced; skip rather than false-positive
      const hasArgsField = /\bargs\s*:/.test(body);
      if (!hasArgsField && !(name in EXEMPTIONS)) {
        violations.push({ name, file: relative(repoRoot, file) });
      }
    }
  }
  return violations;
}

/** Given text and an index just past a struct's opening `{`, returns the body up to (not
 * including) the matching closing `}`, or `null` if braces never balance before EOF. */
function extractBalancedBraceBody(text, startIndex) {
  let depth = 1;
  let i = startIndex;
  for (; i < text.length; i++) {
    if (text[i] === "{") depth++;
    else if (text[i] === "}") {
      depth--;
      if (depth === 0) return text.slice(startIndex, i);
    }
  }
  return null;
}

function main() {
  const files = listRustFiles(srcRoot);
  const violations = findBareMessageOnlyDiagnostics(files);

  if (violations.length > 0) {
    console.error("Found diagnostic/error structs with no `args` field (message-only constructors):\n");
    for (const v of violations) {
      console.error(`  - ${v.name} (${v.file})`);
    }
    console.error(
      "\nAdd `args: crate::diagnostics::DiagnosticArgs` (see docs/i18n/diagnostic-codes.md) or, " +
        "if this struct genuinely never crosses the Tauri boundary as a structured diagnostic, add " +
        "it to EXEMPTIONS in scripts/i18n/check-diagnostic-args.mjs with a reason.",
    );
    process.exitCode = 1;
    return;
  }

  console.log(`No message-only diagnostic/error structs found (${files.length} Rust files scanned).`);
}

const isMain = process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1];
if (isMain) {
  main();
}
