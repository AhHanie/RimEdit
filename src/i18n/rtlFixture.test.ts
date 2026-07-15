// Direction ("dir") fixture tests (docs/i18n/issues/10-formatting-rtl-and-release-tooling.md,
// step 3): "Add `dir` fixture tests using a future RTL metadata fixture without advertising it as
// selectable." `FUTURE_RTL_LOCALE_FIXTURE` is deliberately not added to `SUPPORTED_LOCALES` --
// English remains the only shipped, selectable locale (Plan.md) -- but the underlying
// `lang`/`dir` application mechanism is still exercised end-to-end against a hypothetical RTL
// locale so a real RTL locale can be added later without redesigning this plumbing.

import { afterEach, describe, expect, it } from "vitest";
import { applyDocumentLocaleForMetadata } from "./LocaleProvider";
import { isSupportedLocale, resolveLocale, type LocaleMetadata } from "./locale";

/** A hypothetical future RTL locale. Not part of `SUPPORTED_LOCALES` -- see module doc comment. */
const FUTURE_RTL_LOCALE_FIXTURE: LocaleMetadata = {
  code: "ar",
  direction: "rtl",
  displayName: "Arabic (fixture only, not selectable)",
};

describe("RTL readiness (future-locale fixture)", () => {
  afterEach(() => {
    document.documentElement.lang = "en";
    document.documentElement.dir = "ltr";
  });

  it("applies dir=rtl and the fixture's lang for an RTL locale fixture", () => {
    applyDocumentLocaleForMetadata(FUTURE_RTL_LOCALE_FIXTURE);
    expect(document.documentElement.dir).toBe("rtl");
    expect(document.documentElement.lang).toBe("ar");
  });

  it("applies dir=ltr for an LTR locale", () => {
    applyDocumentLocaleForMetadata({ code: "en", direction: "ltr", displayName: "English" });
    expect(document.documentElement.dir).toBe("ltr");
  });

  it("does not make the RTL fixture selectable through the supported-locale registry", () => {
    expect(isSupportedLocale(FUTURE_RTL_LOCALE_FIXTURE.code)).toBe(false);
    expect(resolveLocale(FUTURE_RTL_LOCALE_FIXTURE.code)).toBe("en");
  });
});
