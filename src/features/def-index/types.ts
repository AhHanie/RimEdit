import type { DiagnosticArgs } from "../../lib/diagnostics";
import type { SourceType } from "../project-settings/types";

export type IndexedSourceKind = "project" | "source";

export interface IndexedDefSource {
  locationId: string;
  locationName: string;
  sourceKind: IndexedSourceKind;
  sourceType: SourceType;
  readOnly: boolean;
  modId?: string;
  gameVersion?: string;
  expansionName?: string;
}

export interface DefIdentityKey {
  defType: string;
  defName: string;
}

export interface IndexedDefField {
  name: string;
  textValue?: string;
  line?: number;
  column?: number;
}

export interface IndexedDef {
  key: DefIdentityKey;
  defType: string;
  defName: string;
  label?: string;
  parentName?: string;
  relativePath: string;
  nodeId?: number;
  line?: number;
  column?: number;
  source: IndexedDefSource;
  fields: IndexedDefField[];
}

export interface DefIndexError {
  locationId: string;
  locationName: string;
  sourceKind: IndexedSourceKind;
  relativePath?: string;
  code: string;
  message: string;
  line?: number;
  column?: number;
  /** Typed, literal interpolation args for `code`. See `src/lib/diagnostics.ts`. */
  args?: DiagnosticArgs;
}

export interface DefIndexSummary {
  indexedDefs: number;
  projectDefs: number;
  sourceDefs: number;
  errors: number;
  builtAtUnixMs: number;
}

export interface DefDuplicateQueryResult {
  projectOccurrences: IndexedDef[];
  sourceOccurrences: IndexedDef[];
  blockingProjectDuplicate: boolean;
  sourceDuplicateWarning: boolean;
}

export interface IndexedDefSearchResult {
  def: IndexedDef;
  rank: number;
}

export interface DefTypeFacet {
  defType: string;
  projectCount: number;
  sourceCount: number;
  totalCount: number;
}

export interface DefIndexFacetSummary {
  defTypes: DefTypeFacet[];
  projectDefs: number;
  sourceDefs: number;
  errors: number;
}

export interface DefReferenceSuggestion {
  defName: string;
  defType: string;
  label: string | null;
  relativePath: string;
  nodeId: number | null;
  line: number | null;
  column: number | null;
  locationId: string;
  locationName: string;
  readOnly: boolean;
  rank: number;
}

export type DefReferenceResolution =
  | { kind: "editableProjectDef"; relativePath: string; nodeId: number | null }
  | { kind: "readOnlySourceDef"; locationId: string; relativePath: string; nodeId: number | null }
  | { kind: "missing" }
  | { kind: "ambiguous" };

export interface DefXmlPreview {
  rawXml: string;
  defLine?: number;
}

export type IndexingPhase = "idle" | "pending" | "running" | "complete" | "failed";

export interface IndexingStatus {
  projectId?: string;
  phase: IndexingPhase;
  pendingFiles: number;
  indexedDefs: number;
  projectDefs: number;
  sourceDefs: number;
  errors: number;
  message?: string;
  updatedAtUnixMs: number;
}
