import { describe, it, expect } from "vitest";
import {
  NAMESPACES,
  flattenStringLeaves,
  extractInterpolationNames,
  findDuplicateKeys,
  generateTranslationKeysSource,
  validateLocaleResources,
} from "./shared.mjs";

describe("flattenStringLeaves", () => {
  it("flattens nested objects into dot paths", () => {
    expect(flattenStringLeaves({ a: { b: "x", c: "y" }, d: "z" })).toEqual({
      "a.b": "x",
      "a.c": "y",
      d: "z",
    });
  });

  it("throws on array leaves", () => {
    expect(() => flattenStringLeaves({ a: ["x"] })).toThrow(/array/);
  });

  it("throws on non-string, non-object leaves", () => {
    expect(() => flattenStringLeaves({ a: 5 })).toThrow(/number/);
  });
});

describe("extractInterpolationNames", () => {
  it("extracts named interpolation placeholders", () => {
    expect(extractInterpolationNames("Hello {{name}}, you have {{count}} items")).toEqual(
      new Set(["name", "count"]),
    );
  });

  it("returns an empty set when there are no placeholders", () => {
    expect(extractInterpolationNames("Hello there")).toEqual(new Set());
  });
});

describe("findDuplicateKeys", () => {
  it("returns no duplicates for well-formed JSON", () => {
    expect(findDuplicateKeys('{"a": "x", "b": {"c": "y"}}')).toEqual([]);
  });

  it("detects a duplicate key at the same nesting level", () => {
    expect(findDuplicateKeys('{"a": "x", "a": "y"}')).toEqual(["a"]);
  });

  it("does not flag identical keys at different nesting levels", () => {
    expect(findDuplicateKeys('{"a": {"a": "x"}, "b": "y"}')).toEqual([]);
  });

  it("ignores colons inside string values", () => {
    expect(findDuplicateKeys('{"a": "x: y", "b": "z"}')).toEqual([]);
  });
});

describe("generateTranslationKeysSource", () => {
  const resourceTexts = {
    common: '{"actions": {"cancel": "Cancel"}}',
    shell: '{"greeting": "Hello {{name}}"}',
  };

  it("emits namespace list, embedded resource literals, and module augmentation for each namespace", () => {
    const source = generateTranslationKeysSource(["common", "shell"], resourceTexts);
    expect(source).toContain('export const namespaces = ["common", "shell"] as const;');
    expect(source).toContain('declare module "i18next"');
    expect(source).toContain("export const enResources = {");
    // No `import x from "*.json"` -- resource content is embedded as a literal object instead, so
    // every string value (including `{{name}}` placeholders) keeps its literal TypeScript type.
    expect(source).not.toMatch(/import .* from ".*\.json"/);
  });

  it("embeds each namespace's parsed JSON content as a literal object, not a bare import", () => {
    const source = generateTranslationKeysSource(["common", "shell"], resourceTexts);
    expect(source).toContain('"cancel": "Cancel"');
    expect(source).toContain('"greeting": "Hello {{name}}"');
  });

  it("is deterministic for the same input", () => {
    const a = generateTranslationKeysSource(["common", "shell"], resourceTexts);
    const b = generateTranslationKeysSource(["common", "shell"], resourceTexts);
    expect(a).toBe(b);
  });
});

describe("validateLocaleResources", () => {
  it("passes for a valid base locale matching the canonical namespace list", () => {
    const resources = {
      en: {
        common: '{"ok": "OK"}',
        shell: "{}",
      },
    };
    expect(validateLocaleResources(resources, ["common", "shell"])).toEqual([]);
  });

  it("fails when the base locale is missing a canonical namespace file", () => {
    const resources = { en: { common: '{"ok": "OK"}' } };
    const errors = validateLocaleResources(resources, ["common", "shell"]);
    expect(errors.some((e) => e.includes('missing namespace file "shell.json"'))).toBe(true);
  });

  it("fails when the base locale has an unlisted namespace file", () => {
    const resources = { en: { common: "{}", extra: "{}" } };
    const errors = validateLocaleResources(resources, ["common"]);
    expect(errors.some((e) => e.includes('unexpected namespace file "extra.json"'))).toBe(true);
  });

  it("fails on invalid JSON", () => {
    const resources = { en: { common: "{ not json" } };
    const errors = validateLocaleResources(resources, ["common"]);
    expect(errors.some((e) => e.includes("Invalid JSON"))).toBe(true);
  });

  it("fails on duplicate keys", () => {
    const resources = { en: { common: '{"ok": "OK", "ok": "Sure"}' } };
    const errors = validateLocaleResources(resources, ["common"]);
    expect(errors.some((e) => e.includes('Duplicate key "ok"'))).toBe(true);
  });

  it("reports missing keys in a non-base locale", () => {
    const resources = {
      en: { common: '{"a": "x", "b": "y"}' },
      fr: { common: '{"a": "x"}' },
    };
    const errors = validateLocaleResources(resources, ["common"]);
    expect(errors.some((e) => e.includes('missing key "common:b"'))).toBe(true);
  });

  it("reports extra keys in a non-base locale", () => {
    const resources = {
      en: { common: '{"a": "x"}' },
      fr: { common: '{"a": "x", "b": "y"}' },
    };
    const errors = validateLocaleResources(resources, ["common"]);
    expect(errors.some((e) => e.includes('extra key "common:b"'))).toBe(true);
  });

  it("reports mismatched interpolation names in a non-base locale", () => {
    const resources = {
      en: { common: '{"greeting": "Hello {{name}}"}' },
      fr: { common: '{"greeting": "Bonjour {{prenom}}"}' },
    };
    const errors = validateLocaleResources(resources, ["common"]);
    expect(errors.some((e) => e.includes("mismatched interpolation names"))).toBe(true);
  });

  it("has no errors for the repo's real canonical namespace list shape", () => {
    expect(NAMESPACES).toEqual(["common", "shell", "settings", "editor", "patches", "diagnostics"]);
  });
});
