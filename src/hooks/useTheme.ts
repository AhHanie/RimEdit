import { useState, useEffect } from "react";
import type { ThemeMode } from "../types/ui";

const STORAGE_KEY = "rimedit-theme-mode";
const VALID_MODES: ThemeMode[] = ["light", "dark", "system"];

function readStoredMode(): ThemeMode {
  const stored = localStorage.getItem(STORAGE_KEY);
  return VALID_MODES.includes(stored as ThemeMode) ? (stored as ThemeMode) : "system";
}

function resolveFromSystem(): "light" | "dark" {
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export interface UseThemeReturn {
  mode: ThemeMode;
  resolvedTheme: "light" | "dark";
  setMode: (mode: ThemeMode) => void;
  cycleMode: () => void;
}

export function useTheme(): UseThemeReturn {
  const [mode, setModeState] = useState<ThemeMode>(readStoredMode);
  const [resolvedTheme, setResolvedTheme] = useState<"light" | "dark">(() =>
    mode === "system" ? resolveFromSystem() : mode,
  );

  useEffect(() => {
    const resolved = mode === "system" ? resolveFromSystem() : mode;
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
    localStorage.setItem(STORAGE_KEY, newMode);
    setModeState(newMode);
  }

  function cycleMode() {
    const next: Record<ThemeMode, ThemeMode> = { light: "dark", dark: "system", system: "light" };
    setMode(next[mode]);
  }

  return { mode, resolvedTheme, setMode, cycleMode };
}
