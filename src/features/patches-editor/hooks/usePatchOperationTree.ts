import { useCallback, useEffect, useRef, useState } from "react";
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
  /** Awaits any in-flight serialize-and-propagate triggered by the most recent `setOperations`
   * call. Callers (the patch editor pane) must await this before save/mode-switch, exactly like
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
    const promise = serializePatchOperations(file)
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

  const flush = useCallback(async () => {
    if (pendingCommitRef.current) {
      await pendingCommitRef.current;
    }
  }, []);

  return { patchFile, loading, error, setOperations, generateId, flush };
}
