export type LocationKind = "project" | "source";

export type SourceType =
  | "baseGame"
  | "localMod"
  | "steamWorkshop"
  | "folder";

export interface RegisteredLocation {
  id: string;
  displayName: string;
  rootPath: string;
  kind: LocationKind;
  sourceType: SourceType;
  readOnly: boolean;
  modId?: string;
  gameVersion?: string;
  /** @deprecated kept for backward-compatibility with existing settings files */
  expansionName?: string;
  createdAt: string;
  updatedAt: string;
}

export interface ProjectSettings {
  schemaVersion: 2;
  gameVersion: string;
  locations: RegisteredLocation[];
  activeProjectId?: string;
}

export interface MissingActiveProjectNotice {
  id: string;
  displayName: string;
  rootPath: string;
}

export interface ProjectSettingsLoadResult {
  settings: ProjectSettings;
  missingActiveProject?: MissingActiveProjectNotice;
}

export interface RegisteredLocationDraft {
  displayName: string;
  rootPath: string;
  kind: LocationKind;
  sourceType: SourceType;
  modId?: string;
  gameVersion?: string;
}

export interface RegisteredLocationUpdate {
  id: string;
  displayName: string;
  sourceType: SourceType;
  modId?: string;
  gameVersion?: string;
}

export interface AppError {
  code: string;
  message: string;
  details?: Record<string, string>;
}
