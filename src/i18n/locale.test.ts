import { describe, it, expect } from "vitest";
import { FALLBACK_LOCALE, SUPPORTED_LOCALES, getLocaleMetadata, isSupportedLocale, resolveLocale } from "./locale";

describe("SUPPORTED_LOCALES", () => {
  it("lists only English in the first release", () => {
    expect(SUPPORTED_LOCALES).toEqual([{ code: "en", direction: "ltr", displayName: "English" }]);
  });
});

describe("isSupportedLocale", () => {
  it("accepts en", () => {
    expect(isSupportedLocale("en")).toBe(true);
  });

  it("rejects an unsupported locale", () => {
    expect(isSupportedLocale("fr")).toBe(false);
  });
});

describe("resolveLocale", () => {
  it("returns the requested locale when supported", () => {
    expect(resolveLocale("en")).toBe("en");
  });

  it("falls back to English for an unsupported locale", () => {
    expect(resolveLocale("fr")).toBe(FALLBACK_LOCALE);
  });

  it("falls back to English for null or undefined", () => {
    expect(resolveLocale(null)).toBe(FALLBACK_LOCALE);
    expect(resolveLocale(undefined)).toBe(FALLBACK_LOCALE);
  });
});

describe("getLocaleMetadata", () => {
  it("returns metadata for a supported locale", () => {
    expect(getLocaleMetadata("en")).toEqual({ code: "en", direction: "ltr", displayName: "English" });
  });

  it("falls back to English metadata for an unknown locale", () => {
    expect(getLocaleMetadata("fr")).toEqual({ code: "en", direction: "ltr", displayName: "English" });
  });
});
