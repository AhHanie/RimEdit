#!/usr/bin/env node
// Generates src/i18n/generated/translation-keys.ts from src/i18n/resources/en/*.json.
// Usage: node scripts/i18n/generate-translation-keys.mjs

import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { NAMESPACES, generateTranslationKeysSource } from "./shared.mjs";

const repoRoot = join(fileURLToPath(import.meta.url), "..", "..", "..");
const resourcesDir = join(repoRoot, "src", "i18n", "resources", "en");
const outputFile = join(repoRoot, "src", "i18n", "generated", "translation-keys.ts");

export function readEnResourceTexts(dir = resourcesDir, namespaces = NAMESPACES) {
  const texts = {};
  for (const ns of namespaces) {
    const path = join(dir, `${ns}.json`);
    if (!existsSync(path)) {
      throw new Error(`Missing resource file: ${path}`);
    }
    texts[ns] = readFileSync(path, "utf8");
  }
  return texts;
}

function main() {
  const texts = readEnResourceTexts();
  const source = generateTranslationKeysSource(NAMESPACES, texts);
  writeFileSync(outputFile, source, "utf8");
  console.log(`Wrote ${outputFile}`);
}

const isMain = process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1];
if (isMain) {
  main();
}
