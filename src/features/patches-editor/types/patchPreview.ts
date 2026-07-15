/** TypeScript mirror of `src-tauri/src/services/patch_preview.rs`'s wire shape, as produced by
 * the `preview_def_patches` Tauri command. See that module's doc comment and
 * `docs/patches-editor/07-preview-engine.md`'s "Implementation Notes" for the scoping decisions
 * this mirrors (top-level-only reorder, full-stream apply, disable/order silently filtered to the
 * selected Def's visible operations). */

import type { DiagnosticArgs } from "../../../lib/diagnostics";

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

/** Mirrors `services::patch_preview::PatchPreviewTarget`. Identifies the exact Def an open editor
 * tab is showing -- its file origin (`locationId` + `relativePath`) and zero-based position among
 * that file's own top-level Defs (`ordinal`) -- independent of `projectId`, which is only the
 * active editable project used as preview context (registered locations, load folders, patch
 * files). `defType`/`identity` (`defName`, or the `Name` attribute for an abstract template) are
 * validation data: the backend re-verifies them against whatever the origin/ordinal resolves to,
 * and refuses to substitute a same-named Def elsewhere if they don't match. */
export interface PatchPreviewTarget {
  locationId: string;
  relativePath: string;
  defType: string;
  identity: string;
  ordinal: number;
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
  /** Compatibility English text mirroring `code`/`args`. Prefer rendering `code`/`args` through
   * `renderDiagnostic` -- see `src-tauri/src/patches/apply.rs`'s `OperationTraceEntry` doc comment. */
  message: string | null;
  code?: string | null;
  /** Typed, literal interpolation args for `code`. See `src/lib/diagnostics.ts`. */
  args?: DiagnosticArgs;
}

export type ApplyDiagnosticSeverity = "error" | "warning";

export interface ApplyDiagnostic {
  severity: ApplyDiagnosticSeverity;
  code: string;
  message: string;
  key: PatchOperationKey | null;
  /** Typed, literal interpolation args for `code`. See `src/lib/diagnostics.ts`. */
  args?: DiagnosticArgs;
}

export type InheritanceDiagnosticSeverity = "error" | "warning";

export interface InheritanceDiagnostic {
  severity: InheritanceDiagnosticSeverity;
  code: string;
  message: string;
  defType: string | null;
  defName: string | null;
  /** Typed, literal interpolation args for `code`. See `src/lib/diagnostics.ts`. */
  args?: DiagnosticArgs;
}

/** One operation affecting the selected Def, in default preview order. */
export interface PatchPreviewOperationSummary {
  key: PatchOperationKey;
  className: string;
  classification: PatchOperationClassification;
  previewSupport: PatchPreviewSupport;
  status: OperationTraceStatus | null;
  /** Compatibility English text mirroring `statusCode`/`statusArgs`. Explains `status` when the
   * apply engine has something more specific to say than the status alone -- e.g. a
   * `PatchOperationFindMod`-wrapped operation skipped because its required mod isn't registered as
   * a location in this project. `null` when `status` is self-explanatory. Prefer rendering
   * `statusCode`/`statusArgs` through `renderDiagnostic` rather than this raw string. */
  statusMessage: string | null;
  /** Stable diagnostic code mirroring `statusMessage`, for `renderDiagnostic`. */
  statusCode?: string | null;
  /** Typed, literal interpolation args for `statusCode`. See `src/lib/diagnostics.ts`. */
  statusArgs?: DiagnosticArgs;
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
  /** Typed, literal interpolation args for `code`. See `src/lib/diagnostics.ts`. */
  args?: DiagnosticArgs;
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
