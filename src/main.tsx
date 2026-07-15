import React from "react";
import ReactDOM from "react-dom/client";
import "./styles/tokens.css";
import "./styles/base.css";
import "./styles/utilities.css";
import { LocaleProvider } from "./i18n/LocaleProvider";
import { resolveLocale } from "./i18n/locale";
import { getProjectSettings, updateAppLocale } from "./features/project-settings";
import App from "./app/App";

// Persists a runtime locale switch to the global host settings. Rejections
// (e.g. a disk write failure) propagate to `changeLocale`'s caller so it can
// keep the app on the previously active locale instead of the failed one.
function persistLocale(locale: string): Promise<void> {
  return updateAppLocale(locale).then(() => undefined);
}

// Resolves the persisted locale before the tree (and, inside it, `AppShell`'s
// locale-sensitive `useSchemaCatalog` call) ever mounts, per Plan.md: "On startup, the settings
// command returns the saved locale before locale-sensitive catalog loading." Called exactly once
// here; `AppShell` receives this same in-flight promise via `initialProjectSettingsPromise` and
// threads it into `useProjectSettings`, so `get_project_settings` -- which has a load-time side
// effect on the backend (clearing a stale active-project notice) -- is never called a second time
// for the same startup.
const projectSettingsPromise = getProjectSettings();

async function bootstrap() {
  let initialLocale: string | undefined;
  try {
    const result = await projectSettingsPromise;
    initialLocale = result.settings.locale;
  } catch {
    // Fall back to LocaleProvider's own default (English). `AppShell` awaits this same
    // (already-rejected) promise through `useProjectSettings` and surfaces the failure as its
    // normal settings-load error -- this bootstrap step never duplicates that handling.
  }

  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <LocaleProvider persistLocale={persistLocale} initialLocale={resolveLocale(initialLocale)}>
        <App initialProjectSettingsPromise={projectSettingsPromise} />
      </LocaleProvider>
    </React.StrictMode>,
  );
}

void bootstrap();
