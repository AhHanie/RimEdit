// This module investigates a claim that i18next's OWN internal active-language state
// (`instance.language`/`instance.resolvedLanguage`, which `useTranslation()`/`t()` read
// directly) can end up on the WRONG locale after two overlapping `changeLanguage(...)` calls
// settle out of issue order -- specifically, that a slow call's internal mutation can land
// *after* a faster, later-issued call has already fully "won" (applied document/state/
// persistence), independent of anything `LocaleProvider`'s own `switchSequenceRef` bookkeeping
// decides (see `LocaleProvider.tsx`, `changeLocale`). The claim: call A (slow, "fr") calls
// `changeLanguage('fr')` first; call B (fast, "de") is issued after A but completes -- including
// its own `changeLanguage('de')` -- before A's `changeLanguage('fr')` promise ever resolves; when
// A's promise finally resolves, i18next's real internal language could still end up "fr", not
// "de", even though B "won" everywhere else (document.lang, provider state, persistence).
//
// This is tested directly against a real i18next instance -- not through `LocaleProvider`, and
// not via a full-method mock of `changeLanguage` like the overlapping-switch tests in
// `LocaleProvider.test.tsx` use -- for two reasons:
//   1. Every existing overlapping-switch test in `LocaleProvider.test.tsx` replaces
//      `changeLanguage` wholesale with `vi.fn()`/`mockImplementationOnce`, which never exercises
//      i18next's real internal state machine, so none of them could have caught (or ruled out)
//      this claim in the first place.
//   2. `LocaleProvider`'s public `changeLocale` resolves every input through `resolveLocale`,
//      which currently collapses everything to the single supported locale ("en" -- see
//      `locale.ts`), so two overlapping `changeLocale` calls can never target two different
//      locales yet. The claimed corruption is only observable when two DIFFERENT target locales
//      are in flight at once, so it has to be exercised one layer down, directly against the
//      instance `changeLocale` wraps, using a custom async backend to control load timing per
//      language (real resource loading, not a stubbed-out method).
//
// Result (see assertions below): i18next already guards this internally, independent of
// anything this codebase does. `changeLanguage(lng)` stamps `this.isLanguageChangingTo = lng`
// synchronously on every call -- a single shared instance property, so the most-recently-issued
// call always overwrites it -- and its resource-load completion callback only applies the result
// (`this.language`, `resolvedLanguage`, the `languageChanged` event) if `this.isLanguageChangingTo`
// STILL equals that specific call's own target at the moment it resolves. A stale call's
// completion therefore always finds that check false (the winning call already reset it, to its
// own target or to `undefined` once applied) and quietly skips applying -- it cannot clobber a
// newer, already-applied result no matter how much later it resolves. This holds regardless of
// resolution order, including with 3+ overlapping calls (verified below), so no additional
// corrective `changeLanguage(localeRef.current)` call is needed in `LocaleProvider`: there is
// nothing for it to correct that i18next has not already protected against at the source.
import i18next from "i18next";
import type { BackendModule, ReadCallback } from "i18next";
import { initReactI18next } from "react-i18next";
import { describe, expect, it } from "vitest";

/** This test constructs a throwaway i18next instance with its own made-up "common:hello" key and
 * locales ("fr"/"de"/"es") that don't exist in this app's real schema -- see the module doc
 * comment above for why. The app globally augments i18next's `CustomTypeOptions` (see
 * `generated/translation-keys.ts`) so every instance's `t()` is normally restricted to real app
 * keys; `RawI18n` is a narrow, untyped view of just the members this test needs, bypassing that
 * augmentation for this instance only, since it is deliberately not using the app's resources. */
interface RawI18n {
  language: string;
  resolvedLanguage?: string;
  isLanguageChangingTo?: string;
  changeLanguage(lng: string): Promise<unknown>;
  t(key: string): string;
}

/** A minimal i18next backend whose `read` for each language stays pending until this test
 * explicitly releases it via the returned `release` map -- lets a test control exactly which
 * `changeLanguage(...)` call's resource load finishes first, independent of call order. */
function createGatedInstance(languages: string[]): { instance: RawI18n; release: Record<string, () => void> } {
  const release: Record<string, () => void> = {};
  const backend: BackendModule = {
    type: "backend",
    init() {},
    read(language: string, _namespace: string, callback: ReadCallback) {
      release[language] = () => callback(null, { hello: `hello-${language}` });
    },
  };

  const instance = i18next.createInstance();
  void instance
    .use(backend)
    .use(initReactI18next)
    .init({
      lng: "en",
      fallbackLng: "en",
      supportedLngs: languages,
      ns: ["common"],
      defaultNS: "common",
      partialBundledLanguages: true,
      resources: { en: { common: { hello: "hello" } } },
      interpolation: { escapeValue: false },
    });

  return { instance: instance as unknown as RawI18n, release };
}

describe("i18next instance: overlapping changeLanguage calls (round 20 finding)", () => {
  it(
    "a slow, earlier-issued call's late-resolving changeLanguage does not overwrite a faster, " +
      "later-issued call's already-applied language",
    async () => {
      const { instance, release } = createGatedInstance(["en", "fr", "de"]);

      // Call A (slow, targets "fr") is issued first...
      const callA = instance.changeLanguage("fr");
      // ...call B (fast, targets "de") is issued second, and is released -- and fully settles --
      // while A's own `changeLanguage` is still pending.
      const callB = instance.changeLanguage("de");
      release.de();
      await callB;

      // B has fully won: instance state already reflects "de" before A has resolved at all.
      expect(instance.language).toBe("de");
      expect(instance.resolvedLanguage).toBe("de");
      expect(instance.t("common:hello")).toBe("hello-de");

      // Now let A's slow changeLanguage finally resolve -- successfully, not a failure. If the
      // claim above held, this is the moment i18next's internal language would silently flip back
      // to "fr", disagreeing with the already-committed "de" state.
      release.fr();
      await callA;

      expect(instance.language).toBe("de");
      expect(instance.resolvedLanguage).toBe("de");
      expect(instance.isLanguageChangingTo).toBeUndefined();
      expect(instance.t("common:hello")).toBe("hello-de");
    },
  );

  it("holds for 3-way overlap regardless of resolution order: the last-issued call always wins", async () => {
    const orders: string[][] = [
      ["fr", "de", "es"],
      ["es", "de", "fr"],
      ["fr", "es", "de"],
    ];

    for (const resolveOrder of orders) {
      const { instance, release } = createGatedInstance(["en", "fr", "de", "es"]);
      const calls: Record<string, Promise<unknown>> = {
        fr: instance.changeLanguage("fr"),
        de: instance.changeLanguage("de"),
        es: instance.changeLanguage("es"),
      };

      for (const lng of resolveOrder) {
        release[lng]();
        await calls[lng];
      }

      // "es" is the last-issued call in every permutation above, regardless of resolution order.
      expect(instance.language).toBe("es");
      expect(instance.t("common:hello")).toBe("hello-es");
    }
  });
});
