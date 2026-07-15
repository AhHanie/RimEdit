import { describe, it, expect } from "vitest";
import { createI18nInstance, initI18n, namespaces } from "./index";

describe("createI18nInstance", () => {
  it("initializes synchronously with English resources", () => {
    const instance = createI18nInstance();
    expect(instance.isInitialized).toBe(true);
    expect(instance.language).toBe("en");
  });

  it("resolves a foundation key from the common namespace", () => {
    const instance = createI18nInstance();
    expect(instance.t("actions.ok", { ns: "common" })).toBe("OK");
  });

  it("interpolates named plural counts", () => {
    const instance = createI18nInstance();
    expect(instance.t("itemCount", { ns: "common", count: 1 })).toBe("1 item");
    expect(instance.t("itemCount", { ns: "common", count: 3 })).toBe("3 items");
  });

  it("loads every canonical namespace", () => {
    const instance = createI18nInstance();
    for (const ns of namespaces) {
      expect(instance.hasResourceBundle("en", ns)).toBe(true);
    }
  });

  it("creates independent instances", () => {
    const a = createI18nInstance();
    const b = createI18nInstance();
    expect(a).not.toBe(b);
  });
});

describe("initI18n", () => {
  it("returns the same shared instance on repeated calls", () => {
    expect(initI18n()).toBe(initI18n());
  });
});
