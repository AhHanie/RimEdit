import { useCallback, useEffect, useRef, useState } from "react";
import { flushSync } from "react-dom";
import { measureAsync } from "../../../instrumentation";
import { parsePatchOperations, serializePatchOperations } from "../api/patchDocument";
import { nextOperationIdForFile } from "../lib/patchOperationDefaults";
import { formatError } from "../../../lib/formatError";
import type { PatchFile, PatchOperationId, PatchOperationNode } from "../types/patchFile";

export interface UsePatchOperationTreeArgs {
  relativePath: string;
  rawXml: string;
  readOnly: boolean;
  onChangeRawXml: (xml: string) => void;
}

export interface UsePatchOperationTreeResult {
  patchFile: PatchFile | null;
  loading: boolean;
  error: string | null;
  /** Replaces the top-level operation list and propagates the resulting XML to the session
   * (debounced only by the in-flight serialize round trip, not by wall-clock time -- see
   * `flush`). */
  setOperations: (updater: (operations: PatchOperationNode[]) => PatchOperationNode[]) => void;
  /** Allocates a fresh, tree-wide-unique operation id for a newly created/duplicated node. */
  generateId: () => PatchOperationId;
  /** Registers a pending-XPath-draft flush callback (see `PatchPathInput`'s staged draft editor).
   * Returns an unregister function. `flush()` calls every registered callback before awaiting its
   * own pending serialize, so a save/mode-switch/navigation never loses an in-progress edit that
   * hasn't hit its idle-commit timer yet. */
  registerDraftFlush: (flush: () => void) => () => void;
  /** Awaits any in-flight serialize-and-propagate triggered by the most recent `setOperations`
   * call, after first committing every registered pending XPath draft (see `registerDraftFlush`).
   * Callers (the patch editor pane) must await this before save/mode-switch, exactly like
   * `useXmlFormController`'s `flushAll` guards the Def form's debounced field commits. */
  flush: () => Promise<void>;
}

/** Owns the client-side editable operation tree for one open `<Patch>` file. The backend AST
 * (`PatchFile`) is the single source of truth for editing; every mutation immediately reserializes
 * to XML text and pushes it into the session's raw XML buffer (`onChangeRawXml`, normally
 * `session.updateRawXml`), reusing that buffer's existing undo/redo, dirty-tracking, and
 * save/save-preview flow unchanged. The tree only reparses from `rawXml` when it changes for a
 * reason other than our own last edit (initial load, undo/redo, or a hand-edit made in raw mode). */
export function usePatchOperationTree({
  relativePath,
  rawXml,
  readOnly,
  onChangeRawXml,
}: UsePatchOperationTreeArgs): UsePatchOperationTreeResult {
  const [patchFile, setPatchFile] = useState<PatchFile | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const lastSyncedRawXmlRef = useRef<string | null>(null);
  const nextIdRef = useRef(0);
  const pendingCommitRef = useRef<Promise<void> | null>(null);
  const commitSeqRef = useRef(0);
  const onChangeRawXmlRef = useRef(onChangeRawXml);
  onChangeRawXmlRef.current = onChangeRawXml;
  const draftFlushersRef = useRef<Set<() => void>>(new Set());

  useEffect(() => {
    if (rawXml === lastSyncedRawXmlRef.current) return;
    let cancelled = false;
    setLoading(true);
    setError(null);
    parsePatchOperations(relativePath, rawXml)
      .then((file) => {
        if (cancelled) return;
        lastSyncedRawXmlRef.current = rawXml;
        nextIdRef.current = nextOperationIdForFile(file);
        setPatchFile(file);
      })
      .catch((e: unknown) => {
        if (!cancelled) setError(formatError(e));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [rawXml, relativePath]);

  const commitToSession = useCallback((file: PatchFile): Promise<void> => {
    const seq = ++commitSeqRef.current;
    const promise = measureAsync("patches.serializeOperationTree", () => serializePatchOperations(file), {
      operationCount: file.operations.length,
    })
      .then((xml) => {
        if (commitSeqRef.current !== seq) return; // superseded by a newer edit
        lastSyncedRawXmlRef.current = xml;
        onChangeRawXmlRef.current(xml);
      })
      .catch((e: unknown) => {
        if (commitSeqRef.current === seq) setError(formatError(e));
      });
    pendingCommitRef.current = promise;
    return promise;
  }, []);

  const setOperations = useCallback(
    (updater: (operations: PatchOperationNode[]) => PatchOperationNode[]) => {
      if (readOnly) return;
      setPatchFile((prev) => {
        if (!prev) return prev;
        const next: PatchFile = { ...prev, operations: updater(prev.operations) };
        void commitToSession(next);
        return next;
      });
    },
    [readOnly, commitToSession],
  );

  const generateId = useCallback(() => nextIdRef.current++, []);

  const registerDraftFlush = useCallback((fn: () => void) => {
    draftFlushersRef.current.add(fn);
    return () => {
      draftFlushersRef.current.delete(fn);
    };
  }, []);

  const flush = useCallback(async () => {
    // Committing a pending draft calls `setOperations`, whose `setPatchFile` updater synchronously
    // records the resulting serialize promise in `pendingCommitRef` (see `commitToSession`) -- but
    // that synchronous-update guarantee only holds for the *first* state update to this hook in a
    // given tick; a second registered draft flushing in the same forEach pass would otherwise have
    // its update deferred to React's next render instead of running immediately. `flushSync` forces
    // every update inside it (and any render they trigger) to complete before returning, so by the
    // time this call returns, `pendingCommitRef` reflects every flushed draft, not just the first.
    if (draftFlushersRef.current.size > 0) {
      flushSync(() => {
        draftFlushersRef.current.forEach((fn) => fn());
      });
    }
    if (pendingCommitRef.current) {
      await pendingCommitRef.current;
    }
  }, []);

  return { patchFile, loading, error, setOperations, generateId, registerDraftFlush, flush };
}
