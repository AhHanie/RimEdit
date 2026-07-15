import { createContext, useCallback, useContext, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { i18n as I18nInstance } from "i18next";
import type { ReactNode } from "react";
import { I18nextProvider } from "react-i18next";
import { initI18n } from "./index";
import {
  FALLBACK_LOCALE,
  getLocaleMetadata,
  resolveLocale,
  type LocaleMetadata,
  type SupportedLocaleCode,
  type TextDirection,
} from "./locale";

/** Injectable persistence hook. Issue 01 wires the plumbing only; no feature
 * calls this yet (persisting/selecting a locale is out of scope until issue 02). */
export type LocalePersistFn = (locale: SupportedLocaleCode) => void | Promise<void>;

export interface LocaleContextValue {
  locale: SupportedLocaleCode;
  direction: TextDirection;
  changeLocale: (locale: string) => Promise<void>;
}

const LocaleContext = createContext<LocaleContextValue | null>(null);

export interface LocaleProviderProps {
  children: ReactNode;
  /** Defaults to the app-wide i18next singleton. Tests should inject an isolated
   * instance (see `src/i18n/testing/renderWithI18n.tsx`) to avoid cross-test state. */
  i18nInstance?: I18nInstance;
  initialLocale?: SupportedLocaleCode;
  persistLocale?: LocalePersistFn;
}

/** Applies `document.documentElement.lang`/`dir` for arbitrary locale metadata. Kept independent
 * of the supported-locale registry (unlike `applyDocumentLocale`) so the underlying `lang`/`dir`
 * mechanism itself -- including RTL directionality -- is directly testable against a future-locale
 * fixture without adding that locale to `SUPPORTED_LOCALES` (which would make it selectable; see
 * `docs/i18n/issues/10-formatting-rtl-and-release-tooling.md` step 3). */
export function applyDocumentLocaleForMetadata(metadata: LocaleMetadata): void {
  if (typeof document === "undefined") return;
  document.documentElement.lang = metadata.code;
  document.documentElement.dir = metadata.direction;
}

function applyDocumentLocale(code: SupportedLocaleCode): void {
  applyDocumentLocaleForMetadata(getLocaleMetadata(code));
}

/** Sets `document.title` from the translated `common:app.name` key, so the title updates on
 * locale change exactly like `lang`/`dir` do above -- this is the same "provider owns document-
 * level locale-dependent state" contract, applied to the one other document-level string that
 * depends on the active locale. `index.html`'s static `<title>` remains the pre-hydration
 * fallback shown before React mounts; this keeps it correct (and updateable) afterward. */
function applyDocumentTitle(i18nInstance: I18nInstance): void {
  if (typeof document === "undefined") return;
  document.title = i18nInstance.t("common:app.name");
}

export function LocaleProvider({
  children,
  i18nInstance,
  initialLocale = FALLBACK_LOCALE as SupportedLocaleCode,
  persistLocale,
}: LocaleProviderProps) {
  const resolvedInstance = useMemo(() => i18nInstance ?? initI18n(), [i18nInstance]);
  const [locale, setLocale] = useState<SupportedLocaleCode>(resolveLocale(initialLocale));
  const persistLocaleRef = useRef(persistLocale);
  persistLocaleRef.current = persistLocale;
  // Tracks the current locale synchronously (unlike `locale` state, which only updates on the
  // next render) so `changeLocale` can always read "the locale in effect right now" without
  // needing `locale` in its own dependency array -- see the revert-on-persist-failure branch
  // below.
  const localeRef = useRef(locale);
  localeRef.current = locale;
  // Guards every shared-state effect in `changeLocale` -- not just the revert-on-persist-failure
  // branch -- against a stale/superseded call. Without this, an overlapping pair of switches --
  // e.g. a slow switch to locale A followed, before A's `changeLanguage`/persistence settle, by a
  // switch to locale B whose OWN (faster) `changeLanguage` + document/state + persistence already
  // completed successfully -- can have A's late `changeLanguage`/persistence resolution (success
  // OR failure) unconditionally reapply/revert to A's locale afterwards, clobbering the newer,
  // already-successful switch to B. Same "stale async response" guard convention as
  // `useCustomFormViews.ts`'s `reloadSequenceRef`: every call captures the counter's value at
  // its own entry, and only acts on it -- at every point past an `await`, since that's exactly
  // where a newer call can start and finish first -- if that value is still the most recently
  // issued one.
  const switchSequenceRef = useRef(0);

  // Layout effects run synchronously after DOM mutation but before the browser
  // paints, so `lang`/`dir`/`title` are correct before the user sees anything.
  useLayoutEffect(() => {
    applyDocumentLocale(locale);
    applyDocumentTitle(resolvedInstance);
  }, [locale, resolvedInstance]);

  const changeLocale = useCallback(
    async (nextLocale: string) => {
      const resolved = resolveLocale(nextLocale);
      const previousLocale = localeRef.current;
      // Captured at entry, before any `await` -- see `switchSequenceRef`'s doc comment above.
      // Any `changeLocale` call that starts later (even one for the same target locale) bumps
      // the shared counter past this value, marking this call permanently stale/superseded from
      // this point on.
      const mySequence = ++switchSequenceRef.current;
      // True only while no newer `changeLocale` call has started since this one captured
      // `mySequence` above. Re-checked after every `await` below -- each is a point where control
      // returns to the event loop and a newer call can start and finish before this call resumes.
      const isCurrent = () => switchSequenceRef.current === mySequence;

      // Plan.md's documented switch order is: set i18next language -> set `<html lang>`/`dir` ->
      // persist. That means the new locale is visibly live before persistence is confirmed, so
      // failure handling is this function's own responsibility, not each caller's: if `persist`
      // rejects, revert i18next/document/state back to `previousLocale` here, deterministically,
      // before rethrowing -- so Plan.md's contract ("failure leaves the previous locale active")
      // holds even if a caller's own error handling does nothing beyond surfacing the message. A
      // caller therefore never needs its own rollback dance, and a second rollback failure (the
      // old failure mode this replaces) can no longer leave the UI stuck on an unpersisted locale.
      await resolvedInstance.changeLanguage(resolved);
      // If a newer `changeLocale` call started (and possibly already finished) while this call's
      // `changeLanguage` was in flight, that newer call now owns document/state/persistence.
      // Applying this (older) call's resolved locale here -- even
      // though its own `changeLanguage` succeeded -- would clobber the newer switch. Stop and
      // resolve quietly: this call's own request was not an error, it was just superseded.
      if (!isCurrent()) return;
      applyDocumentLocale(resolved);
      applyDocumentTitle(resolvedInstance);
      setLocale(resolved);
      localeRef.current = resolved;

      try {
        await persistLocaleRef.current?.(resolved);
      } catch (persistError) {
        // Only revert if THIS call is still the most recently issued switch. If a newer
        // `changeLocale` call has since started (whether it has resolved yet or not), that newer
        // call's outcome -- success or its own revert -- takes precedence, so this stale call's
        // revert must be a no-op. The persist
        // rejection is still surfaced to this call's own caller (it genuinely failed), just
        // without touching shared i18next/document/React state that a newer switch already owns.
        if (isCurrent()) {
          // Best-effort revert against the same primitives that just succeeded moving forward --
          // swallow a revert-side `changeLanguage` rejection (there's nothing further to fall
          // back to) but still restore the document/state that don't reject.
          await resolvedInstance.changeLanguage(previousLocale).catch(() => {});
          // Re-check: a newer call can have started and finished during the revert-side
          // `changeLanguage` await above too, same reasoning as every other await in this
          // function.
          if (isCurrent()) {
            applyDocumentLocale(previousLocale);
            applyDocumentTitle(resolvedInstance);
            setLocale(previousLocale);
            localeRef.current = previousLocale;
          }
        }
        throw persistError;
      }
      // No further shared-state effects follow a successful persist, so no additional guard is
      // needed here: this call's forward apply above already happened while it was still current,
      // and nothing after this point mutates shared state.
    },
    [resolvedInstance],
  );

  const value = useMemo<LocaleContextValue>(
    () => ({
      locale,
      direction: getLocaleMetadata(locale).direction,
      changeLocale,
    }),
    [locale, changeLocale],
  );

  return (
    <I18nextProvider i18n={resolvedInstance}>
      <LocaleContext.Provider value={value}>{children}</LocaleContext.Provider>
    </I18nextProvider>
  );
}

export function useLocale(): LocaleContextValue {
  const value = useContext(LocaleContext);
  if (!value) {
    throw new Error("useLocale must be used within a LocaleProvider");
  }
  return value;
}
