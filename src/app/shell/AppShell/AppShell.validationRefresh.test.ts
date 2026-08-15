import { describe, it, expect } from "vitest";
import { shouldBumpValidationRefreshRevision } from "./AppShell";
import type { IndexingStatus } from "../../../features/def-index";

const ACTIVE_PROJECT_ID = "proj-a";

function status(overrides: Partial<IndexingStatus> = {}): IndexingStatus {
  return {
    projectId: ACTIVE_PROJECT_ID,
    phase: "complete",
    cacheVerification: "verified",
    pendingFiles: 0,
    indexedDefs: 5,
    projectDefs: 5,
    sourceDefs: 0,
    errors: 0,
    updatedAtUnixMs: 0,
    indexBuiltAtUnixMs: 100,
    ...overrides,
  };
}

describe("shouldBumpValidationRefreshRevision", () => {
  it("bumps when the index build timestamp differs from the last observed complete status", () => {
    expect(
      shouldBumpValidationRefreshRevision(
        status({ indexBuiltAtUnixMs: 200 }),
        100,
        ACTIVE_PROJECT_ID,
      ),
    ).toBe(true);
  });

  it("does not bump when the status is not complete", () => {
    expect(
      shouldBumpValidationRefreshRevision(status({ phase: "running" }), 100, ACTIVE_PROJECT_ID),
    ).toBe(false);
  });

  it("bumps for the very first observed complete status (no prior snapshot yet)", () => {
    // A document can already be open -- validated against an empty/partial Interactive-policy
    // fallback index -- before this component has ever observed a "complete" status at all, so
    // the first completion still needs to trigger a catch-up revalidation.
    expect(shouldBumpValidationRefreshRevision(status(), null, ACTIVE_PROJECT_ID)).toBe(true);
  });

  it("does not bump for a mere cache-verification confirmation (checking -> verified, same index)", () => {
    // `confirm_cache_verified` never touches the underlying index, so `indexBuiltAtUnixMs` is
    // identical before and after -- only `cacheVerification` changes.
    const prevBuiltAt = 100;
    const next = status({ cacheVerification: "verified", indexBuiltAtUnixMs: prevBuiltAt });
    expect(shouldBumpValidationRefreshRevision(next, prevBuiltAt, ACTIVE_PROJECT_ID)).toBe(false);
  });

  it("bumps for a genuine rebuild even when def/error counts happen to be unchanged", () => {
    // A rebuild that edits one Def without adding/removing any produces identical counts to the
    // prior complete snapshot, but the index itself was still rebuilt -- a counts-based heuristic
    // would miss this; the build timestamp does not.
    const next = status({ indexedDefs: 5, projectDefs: 5, indexBuiltAtUnixMs: 200 });
    expect(shouldBumpValidationRefreshRevision(next, 100, ACTIVE_PROJECT_ID)).toBe(true);
  });

  it("does not bump when the build timestamp is unchanged across a complete -> complete transition", () => {
    const next = status({ indexBuiltAtUnixMs: 100 });
    expect(shouldBumpValidationRefreshRevision(next, 100, ACTIVE_PROJECT_ID)).toBe(false);
  });

  it("does not bump when indexBuiltAtUnixMs is absent from the status", () => {
    const next = status({ indexBuiltAtUnixMs: undefined });
    expect(shouldBumpValidationRefreshRevision(next, 100, ACTIVE_PROJECT_ID)).toBe(false);
  });

  it("does not bump for a null/undefined status", () => {
    expect(shouldBumpValidationRefreshRevision(null, 100, ACTIVE_PROJECT_ID)).toBe(false);
    expect(shouldBumpValidationRefreshRevision(undefined, 100, ACTIVE_PROJECT_ID)).toBe(false);
  });

  it("does not bump when the status belongs to a project other than the currently active one", () => {
    // Regression: on the render `activeProjectId` first changes, `useIndexingStatus`'s own
    // reset-to-null hasn't taken effect yet, so `indexingStatus` can still be the *previous*
    // project's stale "complete" status. Must not act on it just because it looks like a fresh
    // completion for a `null` baseline.
    const staleStatusFromOtherProject = status({ projectId: "proj-b", indexBuiltAtUnixMs: 999 });
    expect(
      shouldBumpValidationRefreshRevision(staleStatusFromOtherProject, null, ACTIVE_PROJECT_ID),
    ).toBe(false);
  });
});
