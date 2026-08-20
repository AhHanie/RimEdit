import { invoke } from "@tauri-apps/api/core";

/** Reveals the main window once real content has painted into it -- see `src/app/bootstrap.tsx`,
 * which calls this right after its first render commits. `tauri.conf.json`'s window
 * `visible: false` keeps the window hidden (while it still fully renders offscreen) until this
 * fires, which is what avoids the native/WebView2 white flash that a background color alone can't
 * fully suppress on Windows. Rejects harmlessly outside a Tauri shell (e.g. the Vite-only `pnpm dev`
 * server), where there is no native window to show in the first place -- callers should not let
 * that reject the startup sequence. */
export function signalStartupReady(): Promise<void> {
  return invoke("signal_startup_ready");
}
