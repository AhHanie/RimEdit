import "./styles/tokens.css";
import "./styles/base.css";
import "./styles/utilities.css";
// Imported from its own file rather than the `project-settings` barrel: the barrel also
// re-exports `PreferencesDialog` (loaded lazily by `AppShell`), and Rollup treats a barrel's own
// re-export statement as a static edge to that component regardless of which named export an
// importer actually uses -- so importing the barrel here would pull `PreferencesDialog` (and its
// dependents) back into the eagerly-loaded entry chunk.
import { getProjectSettings, updateAppLocale } from "./features/project-settings/api/projectSettings";
import { mountApp } from "./app/bootstrap";

// Persists a runtime locale switch to the global host settings. Rejections
// (e.g. a disk write failure) propagate to `changeLocale`'s caller so it can
// keep the app on the previously active locale instead of the failed one.
function persistLocale(locale: string): Promise<void> {
  return updateAppLocale(locale).then(() => undefined);
}

const startedAtMs = performance.now();

// Resolves the persisted locale before the tree (and, inside it, `AppShell`'s
// locale-sensitive `useSchemaCatalog` call) ever mounts, so the settings command returns the
// saved locale before locale-sensitive catalog loading. Called exactly once
// here; `mountApp` receives this same in-flight promise and threads it into `App`/`AppShell`'s
// `initialProjectSettingsPromise`, so `get_project_settings` -- which has a load-time side effect
// on the backend (clearing a stale active-project notice) -- is never called a second time for the
// same startup.
const projectSettingsPromise = getProjectSettings();

void mountApp({
  container: document.getElementById("root") as HTMLElement,
  projectSettingsPromise,
  persistLocale,
  startedAtMs,
});
