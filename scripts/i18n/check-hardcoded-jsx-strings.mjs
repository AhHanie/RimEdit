#!/usr/bin/env node
// Guards against new hardcoded UI copy landing in JSX after issue 01 established the rule that
// "JSX text, ARIA text, titles, and user-visible errors must use translation descriptors/keys"
// (docs/i18n/issues/01-frontend-foundation.md). Per
// docs/i18n/issues/10-formatting-rtl-and-release-tooling.md step 4: "Add ... no new bare JSX UI
// literals outside approved technical/test fixtures."
//
// This is a source-level heuristic (does a JSX text node / user-facing attribute string literal
// contain a letter), not a semantic translation check -- see checkHardcodedJsxStrings.mjs's module
// doc comment for what counts as "user-facing". A small number of already-reviewed exceptions
// (dev-only/technical/test-fixture strings) are listed in EXEMPTIONS below, exactly like
// scripts/i18n/check-diagnostic-args.mjs's EXEMPTIONS map: adding to this list is a deliberate,
// reviewed decision, not a default escape hatch.
//
// Usage: node scripts/i18n/check-hardcoded-jsx-strings.mjs

import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative, extname } from "node:path";
import { fileURLToPath } from "node:url";
import { findHardcodedJsxStrings } from "./checkHardcodedJsxStrings.mjs";

const repoRoot = join(fileURLToPath(import.meta.url), "..", "..", "..");
const srcRoot = join(repoRoot, "src");

// `{ [relative file path]: Set of exact literal texts allowed in that file }`. Every entry needs a
// reason in this comment block, not just a bare string:
const EXEMPTIONS = {
  // `placeholder` hints showing the exact literal RimWorld XML attribute/package-id syntax the
  // field expects (e.g. "packageId", "mod.a,mod.b") -- these are technical identifier-format
  // examples for a mod author familiar with About.xml/PatchOperation XML, not translatable UI
  // sentences, matching the existing `patch_missing_class_attribute`-style treatment of XML
  // identifiers as literal, never-translated data (see Plan.md's non-goals).
  "src/features/about-editor/components/AboutDependencySection/AboutDependencySection.tsx": new Set([
    "packageId",
    "displayName",
    "downloadUrl",
    "steamWorkshopUrl",
    "old.package.id",
  ]),
  "src/features/about-editor/components/AboutIdentitySection/AboutIdentitySection.tsx": new Set([
    "yourname.modname",
  ]),
  "src/features/about-editor/components/AboutLoadOrderSection/AboutLoadOrderSection.tsx": new Set([
    "package.id",
  ]),
  "src/features/patches-editor/components/PatchOperationForm/PatchOperationForm.tsx": new Set([
    "mod.package.id",
    "mod.a,mod.b",
  ]),
  // The raw-XML-fallback `Class` attribute input -- shown only when the patch operation's class
  // isn't one of the known schema-described variants (see the `select`/`variantNames` branch
  // right above it, which already uses `t("valueFieldRenderer.chooseClass")`). "Class" here is the
  // literal RimWorld XML attribute name, not a sentence.
  "src/features/patches-editor/components/PatchValueEditor/ValueFieldRenderer.tsx": new Set(["Class"]),
};

/** Recursively lists every `.tsx` file under `dir`, excluding test files and the i18n test-utility
 * directory (whose fixtures are explicitly test-only, not shipped UI). */
function listTsxFiles(dir) {
  const out = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    const stat = statSync(full);
    if (stat.isDirectory()) {
      out.push(...listTsxFiles(full));
      continue;
    }
    if (extname(entry) !== ".tsx") continue;
    if (entry.endsWith(".test.tsx")) continue;
    if (relative(srcRoot, full).split(/[\\/]/).includes("testing")) continue;
    out.push(full);
  }
  return out;
}

function main() {
  const files = listTsxFiles(srcRoot);
  const violations = [];

  for (const file of files) {
    const relPath = relative(repoRoot, file).replace(/\\/g, "/");
    const exempt = EXEMPTIONS[relPath];
    const text = readFileSync(file, "utf8");
    for (const found of findHardcodedJsxStrings(file, text)) {
      if (exempt && exempt.has(found.text)) continue;
      violations.push({ ...found, file: relPath });
    }
  }

  if (violations.length > 0) {
    console.error("Found hardcoded UI strings outside approved technical/test fixtures:\n");
    for (const v of violations) {
      console.error(`  - ${v.file}:${v.line}:${v.column} [${v.kind}] "${v.text}"`);
    }
    console.error(
      "\nRoute this text through useTranslation()/t(...) and a src/i18n/resources/en/*.json key, " +
        "or, if this is a genuinely reviewed technical/test-fixture exception, add it to EXEMPTIONS " +
        "in scripts/i18n/check-hardcoded-jsx-strings.mjs with a reason.",
    );
    process.exitCode = 1;
    return;
  }

  console.log(`No hardcoded JSX UI strings found (${files.length} .tsx files scanned).`);
}

const isMain = process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1];
if (isMain) {
  main();
}
