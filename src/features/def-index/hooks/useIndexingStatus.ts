import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { getIndexingStatus, startBackgroundIndexing } from "../api/defIndex";
import type { IndexingStatus } from "../types";

const INDEXING_STATUS_EVENT = "rimedit://indexing-status";

export function useIndexingStatus(
  activeProjectId: string | undefined,
): IndexingStatus | null {
  const [status, setStatus] = useState<IndexingStatus | null>(null);

  // Load initial status on mount
  useEffect(() => {
    getIndexingStatus().then(setStatus).catch(console.error);
  }, []);

  // When the active project changes, kick off background indexing for it
  useEffect(() => {
    if (!activeProjectId) return;
    startBackgroundIndexing(activeProjectId).then(setStatus).catch(console.error);
  }, [activeProjectId]);

  // Subscribe to live status events from the backend
  useEffect(() => {
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
        unlisten = fn;
      })
      .catch(console.error);

    return () => {
      unlisten?.();
    };
  }, [activeProjectId]);

  return status;
}
