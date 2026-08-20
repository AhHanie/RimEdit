import { useState, useEffect } from "react";
import type { ThemeMode } from "../types/ui";
import { readStoredThemeMode, resolveSystemTheme, THEME_STORAGE_KEY } from "./themeResolution";

export interface UseThemeReturn {
  mode: ThemeMode;
  resolvedTheme: "light" | "dark";
  setMode: (mode: ThemeMode) => void;
  cycleMode: () => void;
}

export function useTheme(): UseThemeReturn {
  const [mode, setModeState] = useState<ThemeMode>(readStoredThemeMode);
  const [resolvedTheme, setResolvedTheme] = useState<"light" | "dark">(() =>
    mode === "system" ? resolveSystemTheme() : mode,
  );

  useEffect(() => {
    const resolved = mode === "system" ? resolveSystemTheme() : mode;
    setResolvedTheme(resolved);
  }, [mode]);

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", resolvedTheme);
    document.documentElement.style.colorScheme = resolvedTheme;
  }, [resolvedTheme]);

  useEffect(() => {
    if (mode !== "system") return;

    const mql = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = (e: MediaQueryListEvent) => {
      setResolvedTheme(e.matches ? "dark" : "light");
    };
    mql.addEventListener("change", handler);
    return () => mql.removeEventListener("change", handler);
  }, [mode]);

  function setMode(newMode: ThemeMode) {
    localStorage.setItem(THEME_STORAGE_KEY, newMode);
    setModeState(newMode);
  }

  function cycleMode() {
    const next: Record<ThemeMode, ThemeMode> = { light: "dark", dark: "system", system: "light" };
    setMode(next[mode]);
  }

  return { mode, resolvedTheme, setMode, cycleMode };
}
