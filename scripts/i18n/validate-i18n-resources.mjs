// Validates src/i18n/resources/**/*.json and checks that
// src/i18n/generated/translation-keys.ts is up to date.
// Usage: node scripts/i18n/validate-i18n-resources.mjs

import { readFileSync, readdirSync, existsSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { BASE_LOCALE, NAMESPACES, generateTranslationKeysSource, validateLocaleResources } from "./shared.mjs";

const repoRoot = join(fileURLToPath(import.meta.url), "..", "..", "..");
const resourcesRoot = join(repoRoot, "src", "i18n", "resources");
const generatedFile = join(repoRoot, "src", "i18n", "generated", "translation-keys.ts");

export function readAllLocaleResourceTexts(dir = resourcesRoot) {
  const resourcesByLocale = {};
  for (const locale of readdirSync(dir)) {
    const localeDir = join(dir, locale);
    const files = {};
    for (const entry of readdirSync(localeDir)) {
      if (!entry.endsWith(".json")) continue;
      const ns = entry.slice(0, -".json".length);
      files[ns] = readFileSync(join(localeDir, entry), "utf8");
    }
    resourcesByLocale[locale] = files;
  }
  return resourcesByLocale;
}

export function checkGeneratedFileIsFresh(
  generatedPath = generatedFile,
  namespaces = NAMESPACES,
  enResourceTexts = readAllLocaleResourceTexts()[BASE_LOCALE],
) {
  if (!existsSync(generatedPath)) {
    return [`Generated file is missing: ${generatedPath}. Run \`pnpm i18n:generate\`.`];
  }
  const expected = generateTranslationKeysSource(namespaces, enResourceTexts);
  const actual = readFileSync(generatedPath, "utf8");
  if (expected.replace(/\r\n/g, "\n") !== actual.replace(/\r\n/g, "\n")) {
    return [`${generatedPath} is stale relative to src/i18n/resources/en/*.json. Run \`pnpm i18n:generate\`.`];
  }
  return [];
}

function main() {
  const resourcesByLocale = readAllLocaleResourceTexts();
  const errors = [
    ...validateLocaleResources(resourcesByLocale),
    ...checkGeneratedFileIsFresh(generatedFile, NAMESPACES, resourcesByLocale[BASE_LOCALE]),
  ];

  if (errors.length > 0) {
    console.error("i18n resource validation failed:\n");
    for (const error of errors) {
      console.error(`  - ${error}`);
    }
    process.exitCode = 1;
    return;
  }

  console.log("i18n resources are valid and translation-keys.ts is up to date.");
}

const isMain = process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1];
if (isMain) {
  main();
}
