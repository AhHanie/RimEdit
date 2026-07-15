import { render, screen, waitFor } from "@testing-library/react";
import { useTranslation } from "react-i18next";
import { createI18nInstance } from "./index";
import { LocaleProvider, useLocale } from "./LocaleProvider";
import { renderWithI18n } from "./testing/renderWithI18n";

function Probe() {
  const { t } = useTranslation("common");
  const { locale, direction, changeLocale } = useLocale();
  return (
    <div>
      <span data-testid="translated">{t("actions.ok")}</span>
      <span data-testid="locale">{locale}</span>
      <span data-testid="direction">{direction}</span>
      <button onClick={() => void changeLocale("fr")}>change</button>
    </div>
  );
}

describe("LocaleProvider", () => {
  it("renders translated English text immediately", () => {
    renderWithI18n(<Probe />);
    expect(screen.getByTestId("translated").textContent).toBe("OK");
  });

  it("exposes the current locale and direction via useLocale", () => {
    renderWithI18n(<Probe />);
    expect(screen.getByTestId("locale").textContent).toBe("en");
    expect(screen.getByTestId("direction").textContent).toBe("ltr");
  });

  it("sets document lang and dir", () => {
    renderWithI18n(<Probe />);
    expect(document.documentElement.lang).toBe("en");
    expect(document.documentElement.dir).toBe("ltr");
  });

  it("sets document.title from the translated app name, not the starter-template default", () => {
    renderWithI18n(<Probe />);
    expect(document.title).toBe("RimEdit");
  });

  it("falls back to English when changeLocale is called with an unsupported locale", async () => {
    renderWithI18n(<Probe />);
    const button = screen.getByRole("button", { name: "change" });
    button.click();
    await Promise.resolve();
    expect(screen.getByTestId("locale").textContent).toBe("en");
  });

  it("invokes the injected persistence adapter with the resolved locale", async () => {
    const persistLocale = vi.fn();
    renderWithI18n(<Probe />, { providerProps: { persistLocale } });
    const button = screen.getByRole("button", { name: "change" });
    button.click();
    await Promise.resolve();
    await Promise.resolve();
    expect(persistLocale).toHaveBeenCalledWith("en");
  });

  // `changeLocale` used to apply the new locale to i18next/document/state before awaiting
  // persistence. If persistence failed, that unpersisted state stayed active, and callers were
  // left to attempt their own rollback -- which itself could
  // fail and get silently swallowed, contradicting Plan.md's "failure leaves the previous locale
  // active" contract. `changeLocale` must now guarantee the revert itself.
  it("reverts locale state to the prior value and rejects when persistence fails", async () => {
    const i18nInstance = createI18nInstance();
    const changeLanguageSpy = vi.spyOn(i18nInstance, "changeLanguage");
    const persistLocale = vi.fn().mockRejectedValueOnce(new Error("disk full"));
    let latestChangeLocale: ((locale: string) => Promise<void>) | null = null;

    function Capture() {
      const { locale, changeLocale } = useLocale();
      latestChangeLocale = changeLocale;
      return <span data-testid="locale">{locale}</span>;
    }

    render(
      <LocaleProvider i18nInstance={i18nInstance} persistLocale={persistLocale}>
        <Capture />
      </LocaleProvider>,
    );

    await expect(latestChangeLocale!("en")).rejects.toThrow("disk full");

    // State/document must reflect the prior (still current) locale, not get stuck mid-switch.
    expect(screen.getByTestId("locale").textContent).toBe("en");
    expect(document.documentElement.lang).toBe("en");
    expect(document.documentElement.dir).toBe("ltr");

    // Two calls -- the forward apply, then the revert -- prove the revert branch actually ran,
    // even though both target the same ("en") value, since "en" is the only supported locale
    // today.
    expect(changeLanguageSpy).toHaveBeenCalledTimes(2);

    // A subsequent successful call must still work: the earlier failure must not leave the
    // provider permanently stuck.
    persistLocale.mockResolvedValueOnce(undefined);
    await expect(latestChangeLocale!("en")).resolves.toBeUndefined();
  });

  // The revert-on-failure fix (above) had no generation/sequence guard, so an older,
  // now-superseded `changeLocale` call's late failure could unconditionally revert state a
  // NEWER, already-successful `changeLocale` call had since taken over -- clobbering the newer
  // switch even though it fully succeeded and persisted.
  it("does not let a stale changeLocale call's failed persistence revert a newer, already-successful switch", async () => {
    const i18nInstance = createI18nInstance();
    const changeLanguageSpy = vi.spyOn(i18nInstance, "changeLanguage");

    let rejectA: (err: Error) => void = () => {};
    const persistA = new Promise<void>((_, reject) => {
      rejectA = reject;
    });
    let resolveB: () => void = () => {};
    const persistB = new Promise<void>((resolve) => {
      resolveB = resolve;
    });
    const persistLocale = vi
      .fn()
      .mockReturnValueOnce(persistA)
      .mockReturnValueOnce(persistB);

    let latestChangeLocale: ((locale: string) => Promise<void>) | null = null;
    function Capture() {
      const { locale, changeLocale } = useLocale();
      latestChangeLocale = changeLocale;
      return <span data-testid="locale">{locale}</span>;
    }

    render(
      <LocaleProvider i18nInstance={i18nInstance} persistLocale={persistLocale}>
        <Capture />
      </LocaleProvider>,
    );

    // Two overlapping switches -- call A first (its persistence will reject, but only later),
    // then call B before A's persistence settles. Both target "en" (the only supported locale
    // today; see the previous test's rationale above), so the observable difference between
    // "reverted" and "not reverted" is the number of `changeLanguage` calls, not the resolved
    // locale code.
    //
    // Call A is awaited past its own `changeLanguage` step (i.e. until it has registered its
    // persistence call) before call B starts. This is deliberate: the sequence guard also covers
    // the post-`changeLanguage` checkpoint (see
    // `LocaleProvider.tsx`), so if B started immediately (synchronously back-to-back with A, with
    // no `await` between them), A would already be stale by the time its own `changeLanguage`
    // resolved and would never even reach its persistence call -- which would test the *other*
    // (stale-success) guard, not this failure/revert one. Starting B only after A's persistence
    // call is registered keeps this test focused on the failure/revert branch.
    const callA = latestChangeLocale!("en");
    await waitFor(() => expect(persistLocale).toHaveBeenCalledTimes(1));

    const callB = latestChangeLocale!("en");
    await waitFor(() => expect(persistLocale).toHaveBeenCalledTimes(2));

    // B (the newer switch) succeeds first...
    resolveB();
    await callB;

    // ...then A's (the older, now-stale switch's) persistence finally rejects. Before this fix,
    // `changeLocale`'s catch block unconditionally reverted i18next/document/state back to
    // whatever locale was active before A started -- clobbering B's already-successful,
    // already-persisted switch even though B is fully valid.
    rejectA(new Error("disk full"));
    await expect(callA).rejects.toThrow("disk full");

    // Only the two forward applies (A's and B's) happened -- A's stale revert branch must have
    // been skipped entirely (no third, revert-side `changeLanguage` call).
    expect(changeLanguageSpy).toHaveBeenCalledTimes(2);
    expect(screen.getByTestId("locale").textContent).toBe("en");
  });

  // The guard (above) only covered the revert-on-failure branch, so a stale call's late
  // *successful* `changeLanguage`/persistence could still win. Reproduces the reported race
  // exactly: call A's own `changeLanguage` is slow and still in flight when
  // call B starts and runs to completion (changeLanguage + document/state + persistence) ahead
  // of it. When A's `changeLanguage` finally resolves -- successfully, not a failure -- A must
  // recognize it has been superseded and stop before reapplying/persisting its own (stale)
  // locale over B's already-committed one.
  it("does not let a stale changeLocale call's late successful changeLanguage/persistence overwrite a newer, already-successful switch", async () => {
    const i18nInstance = createI18nInstance();
    const originalChangeLanguage = i18nInstance.changeLanguage.bind(i18nInstance);

    // Call A's `changeLanguage` never settles on its own -- it stays pending until the test
    // explicitly resolves it, simulating "A is slow". Every other call (B's forward call, and any
    // revert-side call) falls through to the real implementation.
    let resolveChangeLanguageA: () => void = () => {};
    const pendingChangeLanguageA = new Promise<void>((resolve) => {
      resolveChangeLanguageA = resolve;
    });
    const changeLanguageSpy = vi
      .spyOn(i18nInstance, "changeLanguage")
      .mockImplementationOnce(() => pendingChangeLanguageA as unknown as ReturnType<typeof originalChangeLanguage>)
      .mockImplementation((...args: Parameters<typeof originalChangeLanguage>) =>
        originalChangeLanguage(...args),
      );

    const persistLocale = vi.fn().mockResolvedValue(undefined);

    let latestChangeLocale: ((locale: string) => Promise<void>) | null = null;
    function Capture() {
      const { locale, changeLocale } = useLocale();
      latestChangeLocale = changeLocale;
      return <span data-testid="locale">{locale}</span>;
    }

    render(
      <LocaleProvider i18nInstance={i18nInstance} persistLocale={persistLocale}>
        <Capture />
      </LocaleProvider>,
    );

    // A starts first and blocks on its own `changeLanguage` call. B starts immediately after
    // (while A is still stuck) and, being unblocked, runs all the way through -- changeLanguage,
    // document/state, and persistence -- before A's `changeLanguage` ever resolves.
    const callA = latestChangeLocale!("en");
    const callB = latestChangeLocale!("en");

    await expect(callB).resolves.toBeUndefined();
    expect(persistLocale).toHaveBeenCalledTimes(1);

    // Now let A's slow `changeLanguage` finally resolve -- successfully. Before this fix, A would
    // have proceeded to reapply its own (stale) locale to document/state and persist it,
    // clobbering B's already-correct, already-persisted switch.
    resolveChangeLanguageA();
    await expect(callA).resolves.toBeUndefined();

    // A must not have persisted again, nor issued any further `changeLanguage` call: it should
    // have recognized staleness right after its own `changeLanguage` resolved and stopped there.
    expect(persistLocale).toHaveBeenCalledTimes(1);
    expect(changeLanguageSpy).toHaveBeenCalledTimes(2);
    expect(screen.getByTestId("locale").textContent).toBe("en");
  });

  it("throws when useLocale is used outside a LocaleProvider", () => {
    function Bare() {
      useLocale();
      return null;
    }
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
    expect(() => render(<Bare />)).toThrow("useLocale must be used within a LocaleProvider");
    consoleError.mockRestore();
  });
});
