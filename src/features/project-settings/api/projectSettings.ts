import { invoke } from "@tauri-apps/api/core";
import type {
  ProjectSettings,
  ProjectSettingsLoadResult,
  RegisteredLocationDraft,
  RegisteredLocationUpdate,
} from "../types";

export function getProjectSettings(): Promise<ProjectSettingsLoadResult> {
  return invoke<ProjectSettingsLoadResult>("get_project_settings");
}

export function upsertLocation(
  draft: RegisteredLocationDraft,
): Promise<ProjectSettings> {
  return invoke<ProjectSettings>("upsert_location", { location: draft });
}

export function removeLocation(id: string): Promise<ProjectSettings> {
  return invoke<ProjectSettings>("remove_location", { id });
}

export function setActiveProject(
  id: string | undefined,
): Promise<ProjectSettings> {
  return invoke<ProjectSettings>("set_active_project", { id: id ?? null });
}

export function updateLocation(
  update: RegisteredLocationUpdate,
): Promise<ProjectSettings> {
  return invoke<ProjectSettings>("update_location", { update });
}

export function updateProjectGameVersion(
  gameVersion: string,
  extraSchemaRoots?: string[],
): Promise<ProjectSettings> {
  return invoke<ProjectSettings>("update_project_game_version", {
    gameVersion,
    extraSchemaRoots,
  });
}

export function listInstalledSchemaGameVersions(
  extraSchemaRoots?: string[],
): Promise<string[]> {
  return invoke<string[]>("list_installed_schema_game_versions_cmd", {
    extraSchemaRoots,
  });
}
