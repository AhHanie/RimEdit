import { renderHook, waitFor } from "@testing-library/react";
import { vi, describe, it, expect, beforeEach, afterEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useIndexingStatus } from "./useIndexingStatus";
import type { IndexingStatus } from "../types";

// Regression for a status-attribution bug: `status` must never describe a *different* project than
// `activeProjectId` while a project switch's own `startBackgroundIndexing` call is still in flight.

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

const invokeMock = vi.mocked(invoke);
const listenMock = vi.mocked(listen);

function status(overrides: Partial<IndexingStatus> = {}): IndexingStatus {
  return {
    phase: "complete",
    cacheVerification: "verified",
    pendingFiles: 0,
    indexedDefs: 1,
    projectDefs: 1,
    sourceDefs: 0,
    errors: 0,
    updatedAtUnixMs: 0,
    ...overrides,
  };
}

describe("useIndexingStatus", () => {
  beforeEach(() => {
    listenMock.mockResolvedValue(() => {});
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("clears status synchronously on an active-project change, instead of showing the previous project's status until the new one resolves", async () => {
    let resolveStartForB!: (v: IndexingStatus) => void;
    invokeMock.mockImplementation((cmd, args) => {
      if (cmd === "get_indexing_status") {
        return Promise.resolve(status({ projectId: "proj-a" }));
      }
      if (cmd === "start_background_indexing") {
        const projectId = (args as { projectId?: string } | undefined)?.projectId;
        if (projectId === "proj-b") {
          return new Promise((res) => {
            resolveStartForB = res;
          });
        }
        return Promise.resolve(status({ projectId }));
      }
      return Promise.reject(new Error(`unexpected command ${String(cmd)}`));
    });

    const { result, rerender } = renderHook(
      ({ projectId }: { projectId: string | undefined }) => useIndexingStatus(projectId),
      { initialProps: { projectId: "proj-a" } },
    );

    await waitFor(() => expect(result.current?.projectId).toBe("proj-a"));

    // Switch to project B -- its own `startBackgroundIndexing` call is still in flight.
    rerender({ projectId: "proj-b" });

    // Must not still be reporting project A's status while B's real status hasn't arrived yet.
    expect(result.current).toBeNull();

    resolveStartForB(status({ projectId: "proj-b" }));
    await waitFor(() => expect(result.current?.projectId).toBe("proj-b"));
  });

  it("ignores a startBackgroundIndexing response describing a different project than the one requested", async () => {
    // The backend's `start_background_indexing` can return a leftover global status for a
    // different project (e.g. `schedule_initialization`'s HydratedHit short-circuit not
    // republishing status for the requested project) -- must not attribute it to this project.
    invokeMock.mockImplementation((cmd, args) => {
      if (cmd === "get_indexing_status") {
        return Promise.resolve(status({ projectId: "proj-a" }));
      }
      if (cmd === "start_background_indexing") {
        const projectId = (args as { projectId?: string } | undefined)?.projectId;
        if (projectId === "proj-b") {
          // Backend returns a status describing proj-a, a leftover from an abandoned job.
          return Promise.resolve(status({ projectId: "proj-a", phase: "running" }));
        }
        return Promise.resolve(status({ projectId }));
      }
      return Promise.reject(new Error(`unexpected command ${String(cmd)}`));
    });

    const { result, rerender } = renderHook(
      ({ projectId }: { projectId: string | undefined }) => useIndexingStatus(projectId),
      { initialProps: { projectId: "proj-a" } },
    );
    await waitFor(() => expect(result.current?.projectId).toBe("proj-a"));

    rerender({ projectId: "proj-b" });

    // Cleared synchronously on the switch, and must stay null -- not become proj-a's status, and
    // not become any other non-null placeholder either -- even after the mismatched response
    // resolves.
    expect(result.current).toBeNull();
    await new Promise((r) => setTimeout(r, 0));
    expect(result.current).toBeNull();
  });

  it("ignores a stale startBackgroundIndexing response that resolves after the user has switched away again", async () => {
    let resolveStartForB!: (v: IndexingStatus) => void;
    invokeMock.mockImplementation((cmd, args) => {
      if (cmd === "get_indexing_status") {
        return Promise.resolve(status({ projectId: "proj-a" }));
      }
      if (cmd === "start_background_indexing") {
        const projectId = (args as { projectId?: string } | undefined)?.projectId;
        if (projectId === "proj-b") {
          return new Promise((res) => {
            resolveStartForB = res;
          });
        }
        return Promise.resolve(status({ projectId }));
      }
      return Promise.reject(new Error(`unexpected command ${String(cmd)}`));
    });

    const { result, rerender } = renderHook(
      ({ projectId }: { projectId: string | undefined }) => useIndexingStatus(projectId),
      { initialProps: { projectId: "proj-a" } },
    );
    await waitFor(() => expect(result.current?.projectId).toBe("proj-a"));

    // Switch A -> B (request in flight), then rapidly back to A before B's response arrives.
    rerender({ projectId: "proj-b" });
    rerender({ projectId: "proj-a" });

    // B's stale response finally resolves -- must not be applied, since the user is back on A.
    resolveStartForB(status({ projectId: "proj-b" }));
    await new Promise((r) => setTimeout(r, 0));
    expect(result.current?.projectId).not.toBe("proj-b");
  });

  it("ignores an older response for the same project when a newer request for it resolves first", async () => {
    // A -> B -> A fires two independent startBackgroundIndexing("proj-a") requests. The backend's
    // spawn_blocking doesn't guarantee completion order matches dispatch order, so the *first*
    // request's response can arrive after the *second* one's already been applied -- project
    // identity alone can't tell these two same-project requests apart; only request order can.
    let resolveCount = 0;
    let resolveFirstA!: (v: IndexingStatus) => void;
    invokeMock.mockImplementation((cmd, args) => {
      if (cmd === "get_indexing_status") {
        return Promise.resolve(status({ projectId: "proj-a", phase: "running" }));
      }
      if (cmd === "start_background_indexing") {
        const projectId = (args as { projectId?: string } | undefined)?.projectId;
        if (projectId === "proj-a") {
          resolveCount += 1;
          if (resolveCount === 1) {
            // First A request: captured while A was still running, resolves late.
            return new Promise((res) => {
              resolveFirstA = res;
            });
          }
          // Second A request (after switching away and back): already complete.
          return Promise.resolve(status({ projectId: "proj-a", phase: "complete" }));
        }
        return Promise.resolve(status({ projectId }));
      }
      return Promise.reject(new Error(`unexpected command ${String(cmd)}`));
    });

    const { result, rerender } = renderHook(
      ({ projectId }: { projectId: string | undefined }) => useIndexingStatus(projectId),
      { initialProps: { projectId: "proj-a" } },
    );

    // Switch away and back to A -- fires a second, independent request for the same project.
    rerender({ projectId: "proj-b" });
    rerender({ projectId: "proj-a" });
    await waitFor(() => expect(result.current?.phase).toBe("complete"));

    // The first (older) A request finally resolves with a stale "running" snapshot -- must not
    // regress the already-applied "complete" status.
    resolveFirstA(status({ projectId: "proj-a", phase: "running" }));
    await new Promise((r) => setTimeout(r, 0));
    expect(result.current?.phase).toBe("complete");
  });

  it("ignores a stale mount-time getIndexingStatus response that resolves after a project's own startBackgroundIndexing already landed", async () => {
    // The mount effect has no particular project in mind -- it just reads whatever the backend's
    // global status happens to be at startup. If a project activates and its own
    // startBackgroundIndexing request resolves first, the later-resolving mount fetch must not
    // overwrite that already-correct status with a stale/irrelevant snapshot.
    let resolveMountFetch!: (v: IndexingStatus) => void;
    invokeMock.mockImplementation((cmd, args) => {
      if (cmd === "get_indexing_status") {
        return new Promise((res) => {
          resolveMountFetch = res;
        });
      }
      if (cmd === "start_background_indexing") {
        const projectId = (args as { projectId?: string } | undefined)?.projectId;
        return Promise.resolve(status({ projectId, phase: "complete" }));
      }
      return Promise.reject(new Error(`unexpected command ${String(cmd)}`));
    });

    const { result, rerender } = renderHook(
      ({ projectId }: { projectId: string | undefined }) => useIndexingStatus(projectId),
      { initialProps: { projectId: undefined as string | undefined } },
    );

    // Activate a project -- its own startBackgroundIndexing request resolves immediately.
    rerender({ projectId: "proj-a" });
    await waitFor(() => expect(result.current?.projectId).toBe("proj-a"));

    // The mount-time fetch (dispatched before any project was active) finally resolves with a
    // stale, unrelated snapshot -- must not be applied over the already-correct status.
    resolveMountFetch(status({ projectId: undefined, phase: "idle" }));
    await new Promise((r) => setTimeout(r, 0));
    expect(result.current?.projectId).toBe("proj-a");
    expect(result.current?.phase).toBe("complete");
  });

  it("ignores a stale startBackgroundIndexing response that resolves after the project was deselected entirely", async () => {
    // Regression: the startBackgroundIndexing effect's `if (!activeProjectId) return;` early
    // return must not skip the token bump -- a transition to "no project active" dispatches no
    // new request, but it still needs to supersede whatever request the *previous* project left
    // in flight, or that stale response would pass the staleness guards unchanged (same project,
    // no newer token) and resurrect the closed project's status.
    let resolveStartForA!: (v: IndexingStatus) => void;
    invokeMock.mockImplementation((cmd, args) => {
      if (cmd === "get_indexing_status") {
        return Promise.resolve(status({ projectId: undefined, phase: "idle" }));
      }
      if (cmd === "start_background_indexing") {
        const projectId = (args as { projectId?: string } | undefined)?.projectId;
        if (projectId === "proj-a") {
          return new Promise((res) => {
            resolveStartForA = res;
          });
        }
        return Promise.resolve(status({ projectId }));
      }
      return Promise.reject(new Error(`unexpected command ${String(cmd)}`));
    });

    const { result, rerender } = renderHook(
      ({ projectId }: { projectId: string | undefined }) => useIndexingStatus(projectId),
      { initialProps: { projectId: "proj-a" as string | undefined } },
    );

    // proj-a's startBackgroundIndexing request is still pending.
    expect(result.current).toBeNull();

    // The project is closed/deselected entirely -- no new request is dispatched.
    rerender({ projectId: undefined });
    expect(result.current).toBeNull();

    // proj-a's stale request finally resolves -- must not resurrect it now that no project is
    // active.
    resolveStartForA(status({ projectId: "proj-a", phase: "complete" }));
    await new Promise((r) => setTimeout(r, 0));
    expect(result.current).toBeNull();
  });

  it("accepts a live hydratingCache event for the active project, before any file-scan status has ever been published", async () => {
    // Regression for Phase 1's fire-and-forget setup: the very first status a project ever sees
    // can now be a `hydratingCache` event pushed straight from the backend's background thread,
    // with no preceding `startBackgroundIndexing`/`getIndexingStatus` response describing it.
    let eventCallback!: (event: { payload: IndexingStatus }) => void;
    listenMock.mockImplementation((_event, cb) => {
      eventCallback = cb as (event: { payload: IndexingStatus }) => void;
      return Promise.resolve(() => {});
    });
    invokeMock.mockImplementation((cmd, args) => {
      if (cmd === "get_indexing_status") {
        return Promise.resolve(status({ projectId: "proj-a" }));
      }
      if (cmd === "start_background_indexing") {
        const projectId = (args as { projectId?: string } | undefined)?.projectId;
        return Promise.resolve(status({ projectId }));
      }
      return Promise.reject(new Error(`unexpected command ${String(cmd)}`));
    });

    const { result } = renderHook(() => useIndexingStatus("proj-a"));
    await waitFor(() => expect(result.current?.projectId).toBe("proj-a"));

    eventCallback({
      payload: status({
        projectId: "proj-a",
        phase: "running",
        cacheVerification: "notRequired",
        currentStage: "hydratingCache",
      }),
    });

    await waitFor(() => expect(result.current?.currentStage).toBe("hydratingCache"));
  });

  it("ignores a live hydratingCache event for a project other than the active one", async () => {
    let eventCallback!: (event: { payload: IndexingStatus }) => void;
    listenMock.mockImplementation((_event, cb) => {
      eventCallback = cb as (event: { payload: IndexingStatus }) => void;
      return Promise.resolve(() => {});
    });
    invokeMock.mockImplementation((cmd, args) => {
      if (cmd === "get_indexing_status") {
        return Promise.resolve(status({ projectId: "proj-a" }));
      }
      if (cmd === "start_background_indexing") {
        const projectId = (args as { projectId?: string } | undefined)?.projectId;
        return Promise.resolve(status({ projectId }));
      }
      return Promise.reject(new Error(`unexpected command ${String(cmd)}`));
    });

    const { result } = renderHook(() => useIndexingStatus("proj-a"));
    await waitFor(() => expect(result.current?.projectId).toBe("proj-a"));

    // A hydration event for a *different* project's scope (e.g. a background thread started for a
    // project that's no longer active by the time it publishes) must not be applied.
    eventCallback({
      payload: status({
        projectId: "proj-b",
        phase: "running",
        currentStage: "hydratingCache",
      }),
    });

    expect(result.current?.projectId).toBe("proj-a");
    expect(result.current?.currentStage).toBeUndefined();
  });

  it("supersedes a stale hydratingCache event for the previous project after switching away", async () => {
    let eventCallback!: (event: { payload: IndexingStatus }) => void;
    listenMock.mockImplementation((_event, cb) => {
      eventCallback = cb as (event: { payload: IndexingStatus }) => void;
      return Promise.resolve(() => {});
    });
    invokeMock.mockImplementation((cmd, args) => {
      if (cmd === "get_indexing_status") {
        return Promise.resolve(status({ projectId: "proj-a" }));
      }
      if (cmd === "start_background_indexing") {
        const projectId = (args as { projectId?: string } | undefined)?.projectId;
        return Promise.resolve(status({ projectId }));
      }
      return Promise.reject(new Error(`unexpected command ${String(cmd)}`));
    });

    const { result, rerender } = renderHook(
      ({ projectId }: { projectId: string | undefined }) => useIndexingStatus(projectId),
      { initialProps: { projectId: "proj-a" as string | undefined } },
    );
    await waitFor(() => expect(result.current?.projectId).toBe("proj-a"));

    eventCallback({
      payload: status({
        projectId: "proj-a",
        phase: "running",
        currentStage: "hydratingCache",
      }),
    });
    await waitFor(() => expect(result.current?.currentStage).toBe("hydratingCache"));

    // Switch away -- clears synchronously, and re-registers the event listener bound to the new
    // activeProjectId ("proj-b").
    rerender({ projectId: "proj-b" });
    expect(result.current).toBeNull();

    // A late hydratingCache event for the now-inactive proj-a arrives (e.g. its own background
    // thread was still mid-hydration when the user switched projects) -- the current listener's
    // project filter (bound to "proj-b") must reject it.
    eventCallback({
      payload: status({
        projectId: "proj-a",
        phase: "running",
        currentStage: "hydratingCache",
      }),
    });

    expect(result.current?.projectId).not.toBe("proj-a");
  });

  it("unsubscribes a late-resolving event listener registration instead of leaking it, if the project switches before listen() resolves", async () => {
    // Regression: `listen()` is itself an async IPC round-trip. If the effect is cleaned up
    // (activeProjectId changes again) before that promise resolves, `unlisten` was still `null` at
    // cleanup time, so `unlisten?.()` was a no-op -- the registration (and its stale-project
    // filter closure) leaked for the rest of the app's life instead of being released.
    invokeMock.mockImplementation((cmd) => {
      if (cmd === "get_indexing_status") {
        return Promise.resolve(status({ projectId: undefined, phase: "idle" }));
      }
      if (cmd === "start_background_indexing") {
        return Promise.resolve(status({ projectId: undefined }));
      }
      return Promise.reject(new Error(`unexpected command ${String(cmd)}`));
    });

    const unlistenForA = vi.fn();
    let resolveListenForA!: (fn: () => void) => void;
    listenMock.mockImplementationOnce(
      () =>
        new Promise((res) => {
          resolveListenForA = res;
        }),
    );

    const { rerender } = renderHook(
      ({ projectId }: { projectId: string | undefined }) => useIndexingStatus(projectId),
      { initialProps: { projectId: "proj-a" as string | undefined } },
    );

    // Switch away from proj-a before its listen() registration has resolved -- the effect's
    // cleanup runs while `unlisten` is still null.
    listenMock.mockResolvedValue(() => {});
    rerender({ projectId: "proj-b" });

    // proj-a's registration finally resolves, after its own effect was already cleaned up -- must
    // be unsubscribed immediately rather than left dangling.
    resolveListenForA(unlistenForA);
    await new Promise((r) => setTimeout(r, 0));
    expect(unlistenForA).toHaveBeenCalledTimes(1);
  });
});
