import { useState, useEffect, useCallback, useRef } from "react";
import type {
  MissingActiveProjectNotice,
  ProjectSettings,
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

export function useProjectSettings(): UseProjectSettingsReturn {
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

  useEffect(() => {
    if (hasLoadedRef.current) return;
    hasLoadedRef.current = true;
    Promise.all([getProjectSettings(), listInstalledSchemaGameVersions()])
      .then(([result, versions]) => {
        setSettings(result.settings);
        setStartupNotice(result.missingActiveProject ?? null);
        setInstalledSchemaVersions(versions);
      })
      .catch((e: unknown) => setError(formatError(e)))
      .finally(() => setLoading(false));
  }, []);

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
    const updated = await updateProjectGameVersion(version);
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
