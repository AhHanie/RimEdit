import type { ThemeMode } from "../types/ui";

export const THEME_STORAGE_KEY = "rimedit-theme-mode";

const VALID_MODES: ThemeMode[] = ["light", "dark", "system"];

export function readStoredThemeMode(): ThemeMode {
  const stored = localStorage.getItem(THEME_STORAGE_KEY);
  return VALID_MODES.includes(stored as ThemeMode) ? (stored as ThemeMode) : "system";
}

export function resolveSystemTheme(): "light" | "dark" {
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

/** Resolves the persisted/system theme synchronously, without React state -- shared by
 * `useTheme` (whose initial `resolvedTheme` state this seeds) and `bootstrap.tsx` (which applies
 * it to `document.documentElement` before the first paint, so the `StartupScreen` fallback never
 * renders in the wrong theme while waiting for `AppShell`/`useTheme` to mount). */
export function resolveInitialTheme(): "light" | "dark" {
  const mode = readStoredThemeMode();
  return mode === "system" ? resolveSystemTheme() : mode;
}
