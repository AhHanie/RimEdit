#!/usr/bin/env node
// CLI wrapper for checkDiagnosticRegistry.mjs -- see that module's doc comment for what this
// checks and why. Per docs/i18n/issues/10-formatting-rtl-and-release-tooling.md step 4: "Add
// scripts that validate ... diagnostic code coverage."
//
// Usage: node scripts/i18n/check-diagnostic-registry.mjs

import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative, extname } from "node:path";
import { fileURLToPath } from "node:url";
import {
  findCodeLikeStringLiterals,
  findOrphanedCatalogCodes,
  findProducedDiagnosticCodes,
  findMissingCatalogCodes,
  isRustTestPath,
} from "./checkDiagnosticRegistry.mjs";

const repoRoot = join(fileURLToPath(import.meta.url), "..", "..", "..");
const rustSrcRoot = join(repoRoot, "src-tauri", "src");
const frontendSrcRoot = join(repoRoot, "src");
const diagnosticsJsonPath = join(repoRoot, "src", "i18n", "resources", "en", "diagnostics.json");

function listFiles(dir, extensions) {
  const out = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    const stat = statSync(full);
    if (stat.isDirectory()) {
      out.push(...listFiles(full, extensions));
    } else if (extensions.includes(extname(entry))) {
      out.push(full);
    }
  }
  return out;
}

function main() {
  const diagnosticsJson = JSON.parse(readFileSync(diagnosticsJsonPath, "utf8"));
  const catalogCodes = Object.keys(diagnosticsJson.codes ?? {});

  // Codes are not exclusively backend-owned (see checkDiagnosticRegistry.mjs's module doc
  // comment), so both trees are scanned for code-like literals.
  const rustFiles = listFiles(rustSrcRoot, [".rs"]);
  const frontendFiles = listFiles(frontendSrcRoot, [".ts", ".tsx"]);
  const allFiles = [...rustFiles, ...frontendFiles];
  const codeLikeStringSets = allFiles.map((file) => findCodeLikeStringLiterals(readFileSync(file, "utf8")));

  const orphaned = findOrphanedCatalogCodes(catalogCodes, codeLikeStringSets);

  // Reverse direction: a code actually constructed in src-tauri/src (via a known family
  // constructor, `.with_code(`, a `code: "..."` struct field, or a `let code = match` block --
  // see findProducedDiagnosticCodes) with no matching catalog entry. Scoped to non-test Rust
  // source only -- test code exercises diagnostic constructors with intentionally fake/example
  // codes that were never meant to need a catalog entry.
  const producedCodes = new Set();
  for (const file of rustFiles) {
    if (isRustTestPath(file)) continue;
    for (const code of findProducedDiagnosticCodes(readFileSync(file, "utf8"), file)) {
      producedCodes.add(code);
    }
  }
  const missing = findMissingCatalogCodes(producedCodes, catalogCodes);

  let failed = false;

  if (orphaned.length > 0) {
    failed = true;
    console.error(
      `Found ${orphaned.length} diagnostics:codes.* entr${orphaned.length === 1 ? "y" : "ies"} in ` +
        `${relative(repoRoot, diagnosticsJsonPath)} with no matching code string anywhere in src-tauri/src or src ` +
        "(likely a renamed/removed diagnostic code left stale in the catalog):\n",
    );
    for (const code of orphaned) {
      console.error(`  - "${code}"`);
    }
    console.error(
      "\nRemove the stale entry, or update it to the code's new name, or confirm it is still produced " +
        "somewhere in src-tauri/src or src if this is a false positive.",
    );
  }

  if (missing.length > 0) {
    failed = true;
    console.error(
      `\nFound ${missing.length} diagnostic code${missing.length === 1 ? "" : "s"} constructed in ` +
        "src-tauri/src with no entry in " +
        `${relative(repoRoot, diagnosticsJsonPath)}'s codes object (renders only via the generic ` +
        "message fallback):\n",
    );
    for (const code of missing) {
      console.error(`  - "${code}"`);
    }
    console.error(
      "\nAdd a catalog entry for the code, or confirm it is a false positive (e.g. a genuinely " +
        "non-wire-facing diagnostic that should be excluded from findProducedDiagnosticCodes).",
    );
  }

  if (failed) {
    process.exitCode = 1;
    return;
  }

  console.log(
    `All ${catalogCodes.length} diagnostics:codes.* entries have a matching code string in src-tauri/src or src ` +
      `(${allFiles.length} files scanned); all ${producedCodes.size} produced diagnostic codes have a catalog entry.`,
  );
}

const isMain = process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1];
if (isMain) {
  main();
}
