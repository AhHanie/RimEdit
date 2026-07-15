// Centralized `Intl` formatting helpers. Locale is always an explicit argument
// (never read from ambient/browser state) so callers stay deterministic and
// testable. These never touch wire/XML/JSON serialization, only display text.

import { FALLBACK_LOCALE } from "./locale";

export interface LocaleFormatOptions {
  locale?: string;
}

export function formatNumber(value: number, options: LocaleFormatOptions & Intl.NumberFormatOptions = {}): string {
  const { locale = FALLBACK_LOCALE, ...rest } = options;
  return new Intl.NumberFormat(locale, rest).format(value);
}

export function formatPercent(value: number, options: LocaleFormatOptions & Intl.NumberFormatOptions = {}): string {
  const { locale = FALLBACK_LOCALE, ...rest } = options;
  return new Intl.NumberFormat(locale, { style: "percent", ...rest }).format(value);
}

export function formatDate(
  value: Date | number,
  options: LocaleFormatOptions & Intl.DateTimeFormatOptions = {},
): string {
  const { locale = FALLBACK_LOCALE, dateStyle = "medium", ...rest } = options;
  return new Intl.DateTimeFormat(locale, { dateStyle, ...rest }).format(value);
}

export function formatTime(
  value: Date | number,
  options: LocaleFormatOptions & Intl.DateTimeFormatOptions = {},
): string {
  const { locale = FALLBACK_LOCALE, timeStyle = "short", ...rest } = options;
  return new Intl.DateTimeFormat(locale, { timeStyle, ...rest }).format(value);
}

export function formatDateTime(
  value: Date | number,
  options: LocaleFormatOptions & Intl.DateTimeFormatOptions = {},
): string {
  const { locale = FALLBACK_LOCALE, dateStyle = "medium", timeStyle = "short", ...rest } = options;
  return new Intl.DateTimeFormat(locale, { dateStyle, timeStyle, ...rest }).format(value);
}

export function formatRelativeTime(
  value: number,
  unit: Intl.RelativeTimeFormatUnit,
  options: LocaleFormatOptions & Intl.RelativeTimeFormatOptions = {},
): string {
  const { locale = FALLBACK_LOCALE, numeric = "auto", ...rest } = options;
  return new Intl.RelativeTimeFormat(locale, { numeric, ...rest }).format(value, unit);
}

const BYTE_UNITS: ReadonlyArray<{ threshold: number; unit: string; divisor: number }> = [
  { threshold: 1024 ** 3, unit: "gigabyte", divisor: 1024 ** 3 },
  { threshold: 1024 ** 2, unit: "megabyte", divisor: 1024 ** 2 },
  { threshold: 1024, unit: "kilobyte", divisor: 1024 },
  { threshold: 0, unit: "byte", divisor: 1 },
];

export function formatFileSize(bytes: number, options: LocaleFormatOptions & Intl.NumberFormatOptions = {}): string {
  const { locale = FALLBACK_LOCALE, ...rest } = options;
  const magnitude = Math.abs(bytes);
  const match = BYTE_UNITS.find((entry) => magnitude >= entry.threshold) ?? BYTE_UNITS[BYTE_UNITS.length - 1];
  const value = bytes / match.divisor;
  return new Intl.NumberFormat(locale, {
    style: "unit",
    unit: match.unit,
    unitDisplay: "short",
    maximumFractionDigits: match.unit === "byte" ? 0 : 1,
    ...rest,
  }).format(value);
}
