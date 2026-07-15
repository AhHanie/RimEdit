import type { DiagnosticArgs } from "../../lib/diagnostics";

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
  schemaVersion: 3;
  gameVersion: string;
  /** Global app UI locale (BCP-47, e.g. "en"). App-wide despite this type's name. */
  locale: string;
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
  /** Typed, literal interpolation args for `code`. See `src/lib/diagnostics.ts`. */
  args?: DiagnosticArgs;
}
