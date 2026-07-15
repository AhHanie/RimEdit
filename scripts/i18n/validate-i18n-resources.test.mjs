import { describe, it, expect } from "vitest";
import { readAllLocaleResourceTexts, checkGeneratedFileIsFresh } from "./validate-i18n-resources.mjs";
import { validateLocaleResources } from "./shared.mjs";

describe("the repo's real i18n resources", () => {
  it("pass full validation against src/i18n/resources", () => {
    const resourcesByLocale = readAllLocaleResourceTexts();
    expect(validateLocaleResources(resourcesByLocale)).toEqual([]);
  });

  it("keep src/i18n/generated/translation-keys.ts up to date", () => {
    expect(checkGeneratedFileIsFresh()).toEqual([]);
  });
});
