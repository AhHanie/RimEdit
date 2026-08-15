import { useState, useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { getIndexingStatus, startBackgroundIndexing } from "../api/defIndex";
import type { IndexingStatus } from "../types";

const INDEXING_STATUS_EVENT = "rimedit://indexing-status";

export function useIndexingStatus(
  activeProjectId: string | undefined,
): IndexingStatus | null {
  const [status, setStatus] = useState<IndexingStatus | null>(null);
  // Monotonic request token (same pattern as `useSchemaCatalog`'s `requestTokenRef`): incremented
  // on every status-fetching dispatch below (both the mount-time `getIndexingStatus` fetch and
  // every `startBackgroundIndexing` call), so a response can be recognized as stale even when it's
  // for the *same* project id as a newer request -- e.g. project A's initial request is still in
  // flight when the user switches to B and back to A again, firing a second, independent request
  // for A. Requests go through the backend's `spawn_blocking`, whose completion order isn't
  // guaranteed to match dispatch order, so comparing only project identity (an earlier attempt)
  // isn't enough to tell the two apart -- an older, possibly-stale response could otherwise
  // overwrite a newer one's already-applied result. Sharing one counter across both effects means
  // the mount fetch and any `startBackgroundIndexing` call correctly invalidate each other,
  // whichever fires (and resolves) first.
  const requestTokenRef = useRef(0);

  // Load initial status on mount. Guarded by the same shared token as `startBackgroundIndexing`
  // below: this fetch has no particular project in mind (it just reads whatever the backend's
  // current global status is at startup), so if a project activates and its own
  // `startBackgroundIndexing` request resolves *before* this one does, this stale mount snapshot
  // must not be allowed to overwrite that already-correct, newer status.
  useEffect(() => {
    const token = ++requestTokenRef.current;
    getIndexingStatus()
      .then((result) => {
        if (requestTokenRef.current !== token) return;
        setStatus(result);
      })
      .catch(console.error);
  }, []);

  // Clear the previous project's status synchronously on every active-project change, before the
  // (async) startBackgroundIndexing call below resolves with the new project's real status:
  // without this, `status` keeps describing the *previous* project for however long that call
  // takes, so any consumer reading it during that window -- e.g. AppShell's
  // shouldBumpValidationRefreshRevision -- would be acting on a status/indexBuiltAtUnixMs that has
  // nothing to do with the project that's actually now active.
  useEffect(() => {
    setStatus(null);
  }, [activeProjectId]);

  // When the active project changes, kick off background indexing for it. The backend decides
  // whether a rebuild is actually needed (e.g. a startup cache hydration already restored a
  // matching index) -- this call is unconditional and always returns the current status.
  useEffect(() => {
    // Increment the token *before* the early return: activeProjectId transitioning to `undefined`
    // (a project closed/deselected) must still supersede whatever request the *previous* project
    // left in flight, even though this transition itself dispatches no new request. Incrementing
    // only inside the `if (activeProjectId)` branch would leave that stale request's token
    // un-superseded, so it would pass the staleness check below unchanged when it eventually
    // resolves and resurrect the closed project's status via `setStatus`.
    const token = ++requestTokenRef.current;
    if (!activeProjectId) return;
    const requestedProjectId = activeProjectId;
    startBackgroundIndexing(requestedProjectId)
      .then((result) => {
        // Two staleness guards, mirroring the live status-event listener below:
        // (1) a newer request (for this project again, or a different one) has since been
        //     dispatched -- `requestTokenRef.current` has moved past this call's own `token`.
        // (2) the backend's response can itself describe a *different* project than the one
        //     requested: `start_background_indexing` returns whatever `DefIndexState`'s single
        //     global status happens to be at return time, and `schedule_initialization`'s
        //     `HydratedHit` short-circuit doesn't always republish status for the requested
        //     project (it can leave a leftover status from whichever project's job last touched
        //     it). Applying that unconditionally would briefly attribute one project's indexing
        //     status to a different, unrelated one.
        if (requestTokenRef.current !== token) return;
        if (result.projectId && result.projectId !== requestedProjectId) return;
        setStatus(result);
      })
      .catch(console.error);
  }, [activeProjectId]);

  // Subscribe to live status events from the backend
  useEffect(() => {
    // `listen()` is itself async (a real IPC round-trip to register the listener), so this effect
    // can be cleaned up (activeProjectId changing again) before it resolves. Without the
    // `cancelled` flag, `unlisten` would still be `null` at cleanup time -- `unlisten?.()` is then
    // a no-op, and the later-resolving `fn` gets assigned into a closure nobody will ever call
    // again, permanently leaking this registration. Its callback would keep filtering against the
    // stale `activeProjectId` it was set up with, so a leftover event for the *old* project (e.g.
    // a job kicked off before the switch, finishing after it) would still pass the filter and
    // resurrect that project's status into `setStatus` indefinitely. `cancelled` lets the
    // resolution handler unsubscribe immediately instead of orphaning the registration.
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    listen<IndexingStatus>(INDEXING_STATUS_EVENT, (event) => {
      // Ignore events for a different project than the currently active one
      if (
        event.payload.projectId &&
        activeProjectId &&
        event.payload.projectId !== activeProjectId
      ) {
        return;
      }
      setStatus(event.payload);
    })
      .then((fn) => {
        if (cancelled) {
          fn();
          return;
        }
        unlisten = fn;
      })
      .catch(console.error);

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [activeProjectId]);

  return status;
}
