import React from "react";
import { createRoot } from "react-dom/client";
// `flushSync` lives on the `react-dom` entry point, not `react-dom/client` -- see this module's
// doc comment for why it's needed here.
import { flushSync } from "react-dom";
import { LocaleProvider } from "../i18n/LocaleProvider";
import { resolveLocale } from "../i18n/locale";
import { markTiming } from "../instrumentation";
import { resolveInitialTheme } from "../hooks/themeResolution";
import { signalStartupReady } from "./commands/windowCommands";
import type { ProjectSettingsLoadResult } from "../features/project-settings";
import App from "./App";
import { StartupScreen } from "./StartupScreen/StartupScreen";

export interface MountAppOptions {
  container: HTMLElement;
  /** The single, already-in-flight `getProjectSettings()` call -- see `main.tsx`'s doc comment on
   * why it must be created exactly once, outside of this function, before it's ever called. */
  projectSettingsPromise: Promise<ProjectSettingsLoadResult>;
  persistLocale?: (locale: string) => Promise<void>;
  /** Defaults to `performance.now()` at call time; overridable so tests/measurements can control
   * the elapsed-time baseline the emitted timing marks are computed against. */
  startedAtMs?: number;
}

/**
 * Owns the one React root for the app's lifetime: mounts `StartupScreen` immediately so the
 * WebView has branded pixels before `getProjectSettings()`
 * resolves, then swaps in the real `LocaleProvider` + `App` tree once the persisted locale is known
 * -- preserving the existing "locale before `AppShell`'s locale-sensitive `useSchemaCatalog` call"
 * and "same settled promise forwarded to `App`" contracts. Each render is flushed synchronously
 * (`flushSync`) so the `markTiming` call immediately after it reflects the actual commit,
 * not merely the moment `root.render` was scheduled (React 18+ batches renders triggered outside an
 * event handler into a microtask by default).
 *
 * The two `<LocaleProvider>` renders below use distinct `key`s (`"startup"`/`"app"`). Without that,
 * React would reconcile the second `root.render` as an *update* to the same `LocaleProvider` fiber
 * (same type, same position) rather than a remount -- and since `LocaleProvider` seeds its locale
 * `useState` from `initialLocale` via a directly-computed (non-lazy) initializer, that initializer
 * only runs on first mount, so the resolved persisted locale from `getProjectSettings()` would be
 * silently dropped. Distinct keys force React to unmount the startup-phase provider and mount a
 * fresh one for `App`, so `initialLocale` is read again.
 *
 * `document.documentElement`'s `data-theme`/`color-scheme` are set synchronously, before the first
 * render, from the same persisted-or-system theme `useTheme` resolves -- `useTheme` itself doesn't
 * run until `AppShell` mounts (after settings resolve), so without this, `StartupScreen` would
 * always paint in the light theme regardless of a saved dark preference or `prefers-color-scheme`,
 * which is exactly the flash/theme mismatch this fallback must avoid.
 *
 * `signalStartupReady()` is fired right after the startup screen's first commit, telling the
 * backend to reveal the (until now hidden -- see `tauri.conf.json`'s `visible: false`) native
 * window. A background-color config alone could not fully suppress a native/WebView2 white flash
 * on Windows (see `src-tauri/src/commands/window.rs`), so the window stays hidden -- rendering
 * offscreen -- until real content already exists to show.
 */
export async function mountApp({
  container,
  projectSettingsPromise,
  persistLocale,
  startedAtMs = performance.now(),
}: MountAppOptions): Promise<void> {
  const root = createRoot(container);

  const initialTheme = resolveInitialTheme();
  document.documentElement.setAttribute("data-theme", initialTheme);
  document.documentElement.style.colorScheme = initialTheme;

  flushSync(() => {
    root.render(
      <React.StrictMode>
        <LocaleProvider key="startup">
          <StartupScreen />
        </LocaleProvider>
      </React.StrictMode>,
    );
  });
  markTiming("app.startup.startupScreenRendered", performance.now() - startedAtMs);
  void signalStartupReady().catch(() => {
    // No native window to show outside a Tauri shell (e.g. the Vite-only dev server) -- expected
    // and harmless there.
  });

  let initialLocale: string | undefined;
  let settingsLoad: "success" | "failure" = "success";
  try {
    const result = await projectSettingsPromise;
    initialLocale = result.settings.locale;
  } catch {
    // Fall back to LocaleProvider's own default (English). `AppShell` awaits this same
    // (already-rejected) promise through `useProjectSettings` and surfaces the failure as its
    // normal settings-load error -- this bootstrap step never duplicates that handling.
    settingsLoad = "failure";
  }

  flushSync(() => {
    root.render(
      <React.StrictMode>
        <LocaleProvider
          key="app"
          persistLocale={persistLocale}
          initialLocale={resolveLocale(initialLocale)}
        >
          <App initialProjectSettingsPromise={projectSettingsPromise} />
        </LocaleProvider>
      </React.StrictMode>,
    );
  });
  markTiming("app.startup.appRendered", performance.now() - startedAtMs, { settingsLoad });
}
