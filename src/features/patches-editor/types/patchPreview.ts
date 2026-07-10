/** TypeScript mirror of `src-tauri/src/services/patch_preview.rs`'s wire shape, as produced by
 * the `preview_def_patches` Tauri command. See that module's doc comment and
 * `docs/patches-editor/07-preview-engine.md`'s "Implementation Notes" for the scoping decisions
 * this mirrors (top-level-only reorder, full-stream apply, disable/order silently filtered to the
 * selected Def's visible operations). */

import type { XPathTarget } from "./xpathCompletion";

export type { XPathTarget };

/** Mirrors `patches::apply::PatchOperationKey`. Identifies one operation (any nesting depth)
 * across the whole preview. */
export interface PatchOperationKey {
  locationId: string;
  relativePath: string;
  operationId: number;
}

export function patchOperationKeyToString(key: PatchOperationKey): string {
  return `${key.locationId} ${key.relativePath} ${key.operationId}`;
}

export function samePatchOperationKey(a: PatchOperationKey, b: PatchOperationKey): boolean {
  return (
    a.locationId === b.locationId &&
    a.relativePath === b.relativePath &&
    a.operationId === b.operationId
  );
}

/** Preview-only per-request overrides. Never persisted to any patch XML file. */
export interface PatchPreviewRequest {
  disabled: PatchOperationKey[];
  order: PatchOperationKey[];
}

/** Mirrors `patches::index::PatchOperationClassification`. */
export type PatchOperationClassification = "builtIn" | "custom" | "unknown";

/** Mirrors `patches::index::PatchPreviewSupport`. */
export type PatchPreviewSupport = { kind: "supported" } | { kind: "unsupported"; reason: string };

/** Mirrors `patches::apply::OperationTraceStatus`. */
export type OperationTraceStatus = "applied" | "failed" | "skipped" | "unsupported";

export interface OperationTraceEntry {
  key: PatchOperationKey;
  className: string;
  status: OperationTraceStatus;
  message: string | null;
}

export type ApplyDiagnosticSeverity = "error" | "warning";

export interface ApplyDiagnostic {
  severity: ApplyDiagnosticSeverity;
  code: string;
  message: string;
  key: PatchOperationKey | null;
}

export type InheritanceDiagnosticSeverity = "error" | "warning";

export interface InheritanceDiagnostic {
  severity: InheritanceDiagnosticSeverity;
  code: string;
  message: string;
  defType: string | null;
  defName: string | null;
}

/** One operation affecting the selected Def, in default preview order. */
export interface PatchPreviewOperationSummary {
  key: PatchOperationKey;
  className: string;
  classification: PatchOperationClassification;
  previewSupport: PatchPreviewSupport;
  status: OperationTraceStatus | null;
  /** Explains `status` when the apply engine has something more specific to say than the status
   * alone -- e.g. a `PatchOperationFindMod`-wrapped operation skipped because its required mod
   * isn't registered as a location in this project. `null` when `status` is self-explanatory. */
  statusMessage: string | null;
  /** Preview-only reorder controls apply only to top-level operations -- see
   * `docs/patches-editor/07-preview-engine.md`'s "Implementation Notes". */
  canReorder: boolean;
  defaultOrder: number;
  fileOrder: number;
  relativePath: string;
  locationId: string;
  locationName: string;
  xpath: string | null;
  /** `target.kind === "unsupported"` means this operation was included only via runtime
   * ancestor-chain correlation, not a statically-known impact-graph match -- render it in a
   * separate "unknown impact" group rather than the normal operation list. */
  target: XPathTarget;
}

export interface PatchPreviewConflictDiagnostic {
  code: string;
  key: PatchOperationKey;
  message: string;
}

export interface PatchPreviewImpactSummary {
  visibleOperationCount: number;
  reorderableOperationCount: number;
  unsupportedOperationCount: number;
  conflictCount: number;
}

export interface PatchPreviewResult {
  /** `null` if no Def matching `defType`/`defName` was found in the combined document. */
  xml: string | null;
  defFound: boolean;
  isPartial: boolean;
  visibleOperations: PatchPreviewOperationSummary[];
  operationTrace: OperationTraceEntry[];
  applyDiagnostics: ApplyDiagnostic[];
  inheritanceDiagnostics: InheritanceDiagnostic[];
  conflictDiagnostics: PatchPreviewConflictDiagnostic[];
  impactSummary: PatchPreviewImpactSummary;
}
