// Supported-locale registry and fallback policy.
// English is the only shipped, selectable locale in the first i18n release;
// this registry exists so later locales/RTL support do not require a wire-format
// or provider redesign.

export type TextDirection = "ltr" | "rtl";

export interface LocaleMetadata {
  code: string;
  direction: TextDirection;
  displayName: string;
}

export const SUPPORTED_LOCALES: readonly LocaleMetadata[] = [
  { code: "en", direction: "ltr", displayName: "English" },
];

export const FALLBACK_LOCALE = "en";

export type SupportedLocaleCode = (typeof SUPPORTED_LOCALES)[number]["code"];

export function isSupportedLocale(code: string): code is SupportedLocaleCode {
  return SUPPORTED_LOCALES.some((locale) => locale.code === code);
}

/** Resolves any input to a supported locale code, falling back to English. */
export function resolveLocale(code: string | null | undefined): SupportedLocaleCode {
  if (code && isSupportedLocale(code)) return code;
  return FALLBACK_LOCALE as SupportedLocaleCode;
}

export function getLocaleMetadata(code: string): LocaleMetadata {
  const match = SUPPORTED_LOCALES.find((locale) => locale.code === code);
  if (match) return match;
  const fallback = SUPPORTED_LOCALES.find((locale) => locale.code === FALLBACK_LOCALE);
  if (!fallback) {
    throw new Error("SUPPORTED_LOCALES must include the fallback locale");
  }
  return fallback;
}
