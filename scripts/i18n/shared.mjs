// Shared pure helpers for the i18n resource generator and validator.
// No filesystem access here so these are easy to unit test with in-memory fixtures.

export const NAMESPACES = ["common", "shell", "settings", "editor", "patches", "diagnostics"];

export const BASE_LOCALE = "en";

const INTERPOLATION_PATTERN = /\{\{\s*([a-zA-Z0-9_]+)\s*\}\}/g;

/**
 * Flattens a nested resource object into dot-path -> string-value pairs.
 * Throws if a leaf is neither a string nor a plain object (arrays are not
 * supported in translation resources).
 */
export function flattenStringLeaves(obj, prefix = "") {
  const out = {};
  for (const [key, value] of Object.entries(obj)) {
    const path = prefix ? `${prefix}.${key}` : key;
    if (value !== null && typeof value === "object" && !Array.isArray(value)) {
      Object.assign(out, flattenStringLeaves(value, path));
    } else if (typeof value === "string") {
      out[path] = value;
    } else {
      throw new Error(
        `Unsupported value at "${path}": expected a string or nested object, got ${Array.isArray(value) ? "array" : typeof value}`,
      );
    }
  }
  return out;
}

/** Extracts the set of `{{name}}` interpolation identifiers used in a string. */
export function extractInterpolationNames(value) {
  const names = new Set();
  let match;
  INTERPOLATION_PATTERN.lastIndex = 0;
  while ((match = INTERPOLATION_PATTERN.exec(value)) !== null) {
    names.add(match[1]);
  }
  return names;
}

/**
 * Scans raw JSON text for duplicate object keys at the same nesting level.
 * `JSON.parse` silently keeps the last occurrence of a duplicate key, so this
 * walks the raw text (tracking string/brace state) instead of the parsed value.
 * Only objects are tracked (this repo's translation resources never use arrays).
 */
export function findDuplicateKeys(rawText) {
  const duplicates = [];
  const frames = [new Map()];
  let i = 0;
  const n = rawText.length;

  while (i < n) {
    const ch = rawText[i];
    if (ch === '"') {
      let str = "";
      i++;
      while (i < n && rawText[i] !== '"') {
        if (rawText[i] === "\\") {
          str += rawText[i] + (rawText[i + 1] ?? "");
          i += 2;
          continue;
        }
        str += rawText[i];
        i++;
      }
      i++;
      let j = i;
      while (j < n && /\s/.test(rawText[j])) j++;
      if (rawText[j] === ":") {
        const frame = frames[frames.length - 1];
        const count = (frame.get(str) ?? 0) + 1;
        frame.set(str, count);
        if (count > 1) duplicates.push(str);
      }
      continue;
    }
    if (ch === "{") {
      frames.push(new Map());
      i++;
      continue;
    }
    if (ch === "}") {
      frames.pop();
      i++;
      continue;
    }
    i++;
  }

  return duplicates;
}

/**
 * Re-serializes one namespace's raw English JSON text as a TypeScript object-literal expression,
 * indented to nest under an `enResources` property. Used by `generateTranslationKeysSource` to
 * embed each namespace's resource content directly in the generated `.ts` file instead of
 * `import`-ing its `.json` file -- see that function's doc comment for why the embedding (not the
 * import) is what makes per-key interpolation-argument type-checking possible at all.
 */
function resourceObjectLiteral(rawJsonText, indent = "  ") {
  const parsed = JSON.parse(rawJsonText);
  return JSON.stringify(parsed, null, 2).split("\n").join(`\n${indent}`);
}

/**
 * Builds the source text for `src/i18n/generated/translation-keys.ts`.
 *
 * `resourceTexts` is `{ [namespace]: rawEnglishJsonText }` (the same shape
 * `generate-translation-keys.mjs`'s `readEnResourceTexts()` / `validate-i18n-resources.mjs`'s
 * `readAllLocaleResourceTexts()[BASE_LOCALE]` already produce). Each namespace's resource object is
 * embedded as a literal TypeScript object (via `resourceObjectLiteral`), NOT `import`-ed from its
 * `.json` file. This is the load-bearing choice for translation-argument type safety (Plan.md's
 * "Translation key and type-safety policy"): TypeScript's `resolveJsonModule` import machinery
 * always widens a JSON string property's type to the general `string` type, discarding the actual
 * `"...{{fieldName}}..."` text; i18next's own advanced TypeScript support (`InterpolationMap` /
 * `ParseInterpolationEntries` in i18next's `typescript/t.d.ts`, shipped since i18next v21+) parses
 * `{{argName}}` placeholders directly out of a resource string's LITERAL type and folds each one
 * into `t()`'s options parameter type as a required named property -- but only when that literal
 * type survives into `CustomTypeOptions.resources`. Embedding the parsed-and-re-stringified JSON as
 * an `as const` object literal (rather than importing the `.json` file) is what preserves that
 * literal type, which is what makes a missing/misnamed interpolation argument a compile error with
 * zero additional application code -- no custom `t` wrapper, no change to any existing `t(...)`
 * call site. Verified empirically: `t("valueEditor.mismatch", {})` for a key requiring
 * `{{actualName}}`/`{{fieldName}}` fails to compile once its namespace is embedded this way, and
 * compiles fine once both are supplied.
 */
export function generateTranslationKeysSource(namespaces, resourceTexts) {
  const resourceEntries = namespaces
    .map((ns) => `  ${ns}: ${resourceObjectLiteral(resourceTexts[ns])},`)
    .join("\n");
  const namespaceArray = namespaces.map((ns) => `"${ns}"`).join(", ");

  return `// AUTO-GENERATED FILE. Do not edit by hand.
// Run \`pnpm i18n:generate\` to regenerate from src/i18n/resources/en/*.json.
//
// Each namespace's English resource object below is embedded as a literal (not imported from its
// \`.json\` file) so every string value keeps its literal TypeScript type, which activates i18next's
// built-in per-key interpolation-argument checking on every \`t(...)\` call site -- see
// \`scripts/i18n/shared.mjs\`'s \`generateTranslationKeysSource\` doc comment for the full mechanism,
// and Plan.md's "Translation key and type-safety policy".

export const defaultNamespace = "common" as const;

export const namespaces = [${namespaceArray}] as const;

export type TranslationNamespace = (typeof namespaces)[number];

export const enResources = {
${resourceEntries}
} as const;

declare module "i18next" {
  interface CustomTypeOptions {
    defaultNS: typeof defaultNamespace;
    resources: typeof enResources;
  }
}
`;
}

/**
 * Validates a set of locale resources.
 *
 * `resourcesByLocale` shape: `{ [locale]: { [namespace]: rawJsonText } }`.
 * Returns a list of human-readable error strings; an empty list means valid.
 */
export function validateLocaleResources(resourcesByLocale, namespaces = NAMESPACES) {
  const errors = [];

  const baseFiles = resourcesByLocale[BASE_LOCALE];
  if (!baseFiles) {
    return [`Missing base locale "${BASE_LOCALE}"`];
  }

  const baseNamespaceSet = new Set(Object.keys(baseFiles));
  for (const ns of namespaces) {
    if (!baseNamespaceSet.has(ns)) {
      errors.push(`Base locale "${BASE_LOCALE}" is missing namespace file "${ns}.json"`);
    }
  }
  for (const ns of baseNamespaceSet) {
    if (!namespaces.includes(ns)) {
      errors.push(`Base locale "${BASE_LOCALE}" has unexpected namespace file "${ns}.json" (add it to the canonical namespace list)`);
    }
  }

  const parsedByLocale = {};
  const flattenedByLocale = {};

  for (const [locale, files] of Object.entries(resourcesByLocale)) {
    parsedByLocale[locale] = {};
    flattenedByLocale[locale] = {};
    for (const [ns, rawText] of Object.entries(files)) {
      for (const duplicate of findDuplicateKeys(rawText)) {
        errors.push(`Duplicate key "${duplicate}" in ${locale}/${ns}.json`);
      }

      let parsed;
      try {
        parsed = JSON.parse(rawText);
      } catch (err) {
        errors.push(`Invalid JSON in ${locale}/${ns}.json: ${err instanceof Error ? err.message : String(err)}`);
        continue;
      }

      parsedByLocale[locale][ns] = parsed;
      try {
        flattenedByLocale[locale][ns] = flattenStringLeaves(parsed);
      } catch (err) {
        errors.push(`${locale}/${ns}.json: ${err instanceof Error ? err.message : String(err)}`);
      }
    }
  }

  const baseFlattened = flattenedByLocale[BASE_LOCALE] ?? {};

  for (const [locale, flattenedNamespaces] of Object.entries(flattenedByLocale)) {
    if (locale === BASE_LOCALE) continue;

    for (const ns of namespaces) {
      const baseKeys = baseFlattened[ns] ?? {};
      const localeKeys = flattenedNamespaces[ns] ?? {};

      for (const key of Object.keys(baseKeys)) {
        if (!(key in localeKeys)) {
          errors.push(`Locale "${locale}" is missing key "${ns}:${key}" present in "${BASE_LOCALE}"`);
          continue;
        }
        const baseNames = extractInterpolationNames(baseKeys[key]);
        const localeNames = extractInterpolationNames(localeKeys[key]);
        const missingNames = [...baseNames].filter((name) => !localeNames.has(name));
        const extraNames = [...localeNames].filter((name) => !baseNames.has(name));
        if (missingNames.length > 0 || extraNames.length > 0) {
          errors.push(
            `Locale "${locale}" key "${ns}:${key}" has mismatched interpolation names ` +
              `(missing: [${missingNames.join(", ")}], extra: [${extraNames.join(", ")}])`,
          );
        }
      }

      for (const key of Object.keys(localeKeys)) {
        if (!(key in baseKeys)) {
          errors.push(`Locale "${locale}" has extra key "${ns}:${key}" not present in "${BASE_LOCALE}"`);
        }
      }
    }
  }

  return errors;
}
