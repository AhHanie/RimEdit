import { describe, it, expect } from "vitest";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative, extname } from "node:path";
import { fileURLToPath } from "node:url";
import { findHardcodedJsxStrings } from "./checkHardcodedJsxStrings.mjs";

const repoRoot = join(fileURLToPath(import.meta.url), "..", "..", "..");
const srcRoot = join(repoRoot, "src");

// Mirrors check-hardcoded-jsx-strings.mjs's own EXEMPTIONS -- kept as a separate literal here
// (rather than importing the CLI's private constant) so this integration test independently
// proves every real violation in the committed tree is either fixed or a reviewed exemption, not
// just that the CLI script agrees with itself.
const EXEMPTIONS = {
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
  "src/features/patches-editor/components/PatchValueEditor/ValueFieldRenderer.tsx": new Set(["Class"]),
};

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

describe("check-hardcoded-jsx-strings (real tree)", () => {
  it("finds no un-exempted hardcoded JSX UI strings under src/", () => {
    const violations = [];
    for (const file of listTsxFiles(srcRoot)) {
      const relPath = relative(repoRoot, file).replace(/\\/g, "/");
      const exempt = EXEMPTIONS[relPath];
      const text = readFileSync(file, "utf8");
      for (const found of findHardcodedJsxStrings(file, text)) {
        if (exempt && exempt.has(found.text)) continue;
        violations.push({ ...found, file: relPath });
      }
    }
    expect(violations).toEqual([]);
  });
});
