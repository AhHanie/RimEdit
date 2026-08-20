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

export interface DefIndexErrorsResponse {
  errors: DefIndexError[];
  total: number;
  truncated: boolean;
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

export type IndexingStage = "hydratingCache" | "discovering" | "indexing";

/** Whether the def-index cache backing the current status has been verified against the actual
 * collection on disk. A fast hydrated-cache startup reports "checking" (real Def/error counts,
 * but not yet fingerprint-verified); a background verification flips a matching status to
 * "verified" on a match, or triggers a rebuild on a mismatch that itself ends "verified". Every
 * other status (explicit settings-triggered/manual rebuilds while in progress) reports
 * "notRequired". */
export type CacheVerification = "notRequired" | "checking" | "verified";

export interface IndexingStatus {
  projectId?: string;
  phase: IndexingPhase;
  cacheVerification: CacheVerification;
  pendingFiles: number;
  indexedDefs: number;
  projectDefs: number;
  sourceDefs: number;
  errors: number;
  message?: string;
  updatedAtUnixMs: number;
  /** Total files discovered for the current full rebuild. Set once the `discovering` stage
   * completes; absent before then, during incremental jobs, or once `complete`/`failed`. */
  totalFiles?: number;
  processedFiles?: number;
  currentStage?: IndexingStage;
  /** Display name only -- never a filesystem path. */
  currentLocationName?: string;
  /** The completed index's own build timestamp, present only on a "complete" status. Distinct
   * from `updatedAtUnixMs` (which changes on every status write, including a mere cache-
   * verification confirmation that leaves the underlying index untouched): this only changes when
   * the index itself was actually (re)built, so it's the signal for "did the collection's data
   * actually change" -- see `shouldBumpValidationRefreshRevision`. */
  indexBuiltAtUnixMs?: number;
}
