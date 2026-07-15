import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import type {
  MissingActiveProjectNotice,
  ProjectSettings,
  ProjectSettingsLoadResult,
  RegisteredLocationDraft,
  RegisteredLocationUpdate,
} from "../types";
import {
  getProjectSettings,
  upsertLocation,
  removeLocation,
  setActiveProject,
  updateLocation,
  updateProjectGameVersion,
  listInstalledSchemaGameVersions,
} from "../api/projectSettings";
import { formatError } from "../../../lib/formatError";

interface UseProjectSettingsReturn {
  settings: ProjectSettings | null;
  loading: boolean;
  error: string | null;
  installedSchemaVersions: string[];
  startupNotice: MissingActiveProjectNotice | null;
  clearStartupNotice: () => void;
  addLocation: (draft: RegisteredLocationDraft) => Promise<void>;
  deleteLocation: (id: string) => Promise<void>;
  editLocation: (update: RegisteredLocationUpdate) => Promise<void>;
  activateProject: (id: string | undefined) => Promise<void>;
  updateGameVersion: (version: string) => Promise<void>;
  replaceSettings: (settings: ProjectSettings) => void;
}

/**
 * @param initialLoad When provided, the initial-load effect awaits this promise instead of
 * calling `getProjectSettings()` itself. `main.tsx` calls `getProjectSettings()` exactly once,
 * before mounting `LocaleProvider`, so the persisted locale is resolved and applied before any
 * locale-sensitive catalog request fires (Plan.md: "the settings command returns the saved locale
 * before locale-sensitive catalog loading"); threading that same in-flight promise down here
 * lets `AppShell` consume its result too without a second `get_project_settings` call, which
 * would silently re-run (and lose) the load-time missing-active-project side effect described
 * below. Omitted (e.g. in every existing test) it falls back to calling `getProjectSettings()`
 * directly, unchanged from before.
 */
export function useProjectSettings(
  initialLoad?: Promise<ProjectSettingsLoadResult>,
): UseProjectSettingsReturn {
  const [settings, setSettings] = useState<ProjectSettings | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [installedSchemaVersions, setInstalledSchemaVersions] = useState<string[]>([]);
  const [startupNotice, setStartupNotice] =
    useState<MissingActiveProjectNotice | null>(null);
  // get_project_settings persists the missing-active-project deactivation as a
  // side effect of loading, so calling it twice (e.g. React StrictMode's
  // double-invoked mount effect in dev) would have the second call silently
  // re-read already-cleared settings and overwrite the notice with null. Guard
  // against that by only ever issuing the load once per component lifetime.
  const hasLoadedRef = useRef(false);

  // Issue 09 finding: game-version discovery/selection must search every registered location's
  // root as a candidate external-schema-pack root (mirrors `AppShell`'s `extraSchemaRoots`
  // derivation, `services::validation::schema_pack_roots` on the backend, and
  // `patch_preview::preview_def_for_project`'s existing pattern) -- otherwise a project whose
  // ONLY source of some game version is a mod-embedded schema pack would never see that version
  // offered in the selector at all, and could never select it even by typing it in. Kept as a
  // ref (not a plain closed-over value) so `updateGameVersion` below always reads the CURRENT
  // locations at call time without needing `settings` as a dependency (which would recreate the
  // callback -- and every consumer's memoized callback prop -- on every settings change).
  const settingsRef = useRef<ProjectSettings | null>(null);
  settingsRef.current = settings;

  // Tracks the root-paths key (see `locationRootsKey` below) that installed-schema-version
  // discovery was LAST actually fetched with -- `null` until the very first fetch (from the
  // initial-load effect) has recorded one. This is the single source of truth both effects below
  // coordinate through, replacing a plain "is this the first run" boolean: a boolean can't tell
  // apart "the reactive effect's very first run, which raced ahead of the initial load and saw
  // stale/empty roots" from "the reactive effect's first run AFTER the initial load populated
  // real roots" -- both looked like "first run" to a boolean, but only the former should be
  // skipped. Comparing against the actual last-fetched key handles both correctly and prevents
  // an unnecessary duplicate `listInstalledSchemaGameVersions` scan on every app/project load.
  const lastFetchedRootsKeyRef = useRef<string | null>(null);

  useEffect(() => {
    if (hasLoadedRef.current) return;
    hasLoadedRef.current = true;
    // Sequenced (not `Promise.all`-parallel like before): the roots passed to
    // `listInstalledSchemaGameVersions` must come from the just-loaded settings' own
    // registered locations, so settings must resolve first.
    (async () => {
      try {
        const result = await (initialLoad ?? getProjectSettings());
        setSettings(result.settings);
        setStartupNotice(result.missingActiveProject ?? null);
        const roots = result.settings.locations.map((l) => l.rootPath);
        // Recorded before the (also-async) version fetch below, and before this function yields
        // back to the event loop -- so by the time `setSettings` above triggers a re-render,
        // this ref is already correct and the reactive effect's comparison (which only runs
        // afterward, once React actually re-renders) can never race it.
        lastFetchedRootsKeyRef.current = JSON.stringify(roots);
        const versions = await listInstalledSchemaGameVersions(roots);
        setInstalledSchemaVersions(versions);
      } catch (e: unknown) {
        setError(formatError(e));
      } finally {
        setLoading(false);
      }
    })();
  }, []);

  // Re-derive installed schema versions whenever the registered-location set actually changes
  // content (add/remove/edit a location, e.g. adding a source folder with an embedded
  // `SchemaPacks/` folder) -- not on every unrelated settings change (active project switch,
  // etc.), via the root-paths key below. `JSON.stringify` (not a joined string) avoids both an
  // ambiguous delimiter and a raw non-printable delimiter ending up in source -- see issue 05's
  // analogous `getVisibilityId` fix.
  const locationRootsKey = useMemo(
    () => JSON.stringify(settings?.locations.map((l) => l.rootPath) ?? []),
    [settings?.locations],
  );
  useEffect(() => {
    if (!hasLoadedRef.current) return;
    // The initial-load effect hasn't recorded a fetched key yet (it's still awaiting
    // `getProjectSettings()`) -- this run corresponds to the pre-load render, not a real
    // location-set change, so let the initial-load effect's own fetch be the only one.
    if (lastFetchedRootsKeyRef.current === null) return;
    // Already fetched for this exact root set (e.g. the initial load's fetch just completed and
    // triggered this effect to re-run via its own `setSettings`) -- nothing changed, skip.
    if (locationRootsKey === lastFetchedRootsKeyRef.current) return;
    lastFetchedRootsKeyRef.current = locationRootsKey;
    const roots = settingsRef.current?.locations.map((l) => l.rootPath) ?? [];
    listInstalledSchemaGameVersions(roots)
      .then((versions) => setInstalledSchemaVersions(versions))
      .catch((e: unknown) => setError(formatError(e)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [locationRootsKey]);

  const clearStartupNotice = useCallback(() => {
    setStartupNotice(null);
  }, []);

  const addLocation = useCallback(async (draft: RegisteredLocationDraft) => {
    const updated = await upsertLocation(draft);
    setSettings(updated);
  }, []);

  const deleteLocation = useCallback(async (id: string) => {
    const updated = await removeLocation(id);
    setSettings(updated);
    setStartupNotice(null);
  }, []);

  const activateProject = useCallback(async (id: string | undefined) => {
    const updated = await setActiveProject(id);
    setSettings(updated);
    setStartupNotice(null);
  }, []);

  const editLocation = useCallback(async (update: RegisteredLocationUpdate) => {
    const updated = await updateLocation(update);
    setSettings(updated);
  }, []);

  const updateGameVersion = useCallback(async (version: string) => {
    // Reads the CURRENT locations via `settingsRef` (not a `settings` dependency -- see that
    // ref's doc comment) so the backend's "is this version actually installed" check
    // (`update_project_game_version` -> `list_installed_schema_game_versions`) considers the
    // same external-pack roots as everything else, and doesn't reject a version that's only
    // available via a registered location's embedded schema pack.
    const roots = settingsRef.current?.locations.map((l) => l.rootPath) ?? [];
    const updated = await updateProjectGameVersion(version, roots);
    setSettings(updated);
  }, []);

  const replaceSettings = useCallback((next: ProjectSettings) => {
    setSettings(next);
    setStartupNotice(null);
  }, []);

  return {
    settings,
    loading,
    error,
    installedSchemaVersions,
    startupNotice,
    clearStartupNotice,
    addLocation,
    deleteLocation,
    editLocation,
    activateProject,
    updateGameVersion,
    replaceSettings,
  };
}
