import { describe, it, expect } from "vitest";
import { createI18nInstance } from "../../../i18n/index";
import { renderGraphicPreviewLabel } from "./graphicPreviewLabels";

describe("renderGraphicPreviewLabel", () => {
  it("renders single", () => {
    const i18n = createI18nInstance();
    expect(renderGraphicPreviewLabel({ kind: "single" }, i18n)).toBe("Single");
  });

  it("renders a bare direction", () => {
    const i18n = createI18nInstance();
    expect(renderGraphicPreviewLabel({ kind: "direction", direction: "north" }, i18n)).toBe("North");
    expect(renderGraphicPreviewLabel({ kind: "direction", direction: "east" }, i18n)).toBe("East");
    expect(renderGraphicPreviewLabel({ kind: "direction", direction: "south" }, i18n)).toBe("South");
    expect(renderGraphicPreviewLabel({ kind: "direction", direction: "west" }, i18n)).toBe("West");
  });

  it("renders a variant index, with and without a direction suffix", () => {
    const i18n = createI18nInstance();
    expect(renderGraphicPreviewLabel({ kind: "variant", index: 2 }, i18n)).toBe("Variant 2");
    expect(
      renderGraphicPreviewLabel({ kind: "variant", index: 1, direction: "north" }, i18n),
    ).toBe("Variant 1 North");
  });

  it("renders each stack slot, with and without a direction suffix", () => {
    const i18n = createI18nInstance();
    expect(renderGraphicPreviewLabel({ kind: "stack", stack: "single" }, i18n)).toBe("Stack 1");
    expect(renderGraphicPreviewLabel({ kind: "stack", stack: "partial" }, i18n)).toBe("Stack partial");
    expect(renderGraphicPreviewLabel({ kind: "stack", stack: "full" }, i18n)).toBe("Stack full");
    expect(
      renderGraphicPreviewLabel({ kind: "stack", stack: "partial", direction: "west" }, i18n),
    ).toBe("Stack partial West");
  });

  it("renders an appearance index", () => {
    const i18n = createI18nInstance();
    expect(renderGraphicPreviewLabel({ kind: "appearance", index: 3 }, i18n)).toBe("Appearance 3");
  });

  it("interpolates an appearance-named suffix verbatim, never as a translation-catalog lookup", () => {
    const i18n = createI18nInstance();
    // A file-name-derived suffix is arbitrary mod-author text -- it must never be looked up in
    // the catalog (which would either mistranslate it or fall back to the raw string anyway);
    // it must pass through unchanged.
    expect(renderGraphicPreviewLabel({ kind: "appearanceNamed", suffix: "Damaged" }, i18n)).toBe("Damaged");
  });
});
