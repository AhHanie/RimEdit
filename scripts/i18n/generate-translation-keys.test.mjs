import { describe, it, expect } from "vitest";
import { readEnResourceTexts } from "./generate-translation-keys.mjs";
import { NAMESPACES } from "./shared.mjs";

describe("readEnResourceTexts", () => {
  it("reads every canonical namespace file from the real resources/en directory", () => {
    const texts = readEnResourceTexts();
    expect(Object.keys(texts)).toEqual(NAMESPACES);
    for (const ns of NAMESPACES) {
      expect(() => JSON.parse(texts[ns])).not.toThrow();
    }
  });
});
