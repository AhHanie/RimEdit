import { useCallback, useEffect, useRef, useState } from "react";
import {
  applyXmlEditorEdit,
  applyXmlEditorEdits,
  createDefFromTemplate,
  parseXmlEditorBuffer,
  readLocationXmlEditorDocument,
  readProjectXmlEditorDocument,
} from "../api/xmlDocument";
import {
  createDefFromIndexedDef,
  createDefFromUserTemplate,
  deleteUserDefTemplate as deleteUserDefTemplateApi,
  listUserDefTemplates as listUserDefTemplatesApi,
  saveUserDefTemplate,
} from "../api/defTemplates";
import { previewProjectXmlSave, saveProjectXmlFile } from "../api/projectSave";
import { formatError } from "../../../lib/formatError";
import {
  applyLineEnding,
  detectLineEnding,
  type LineEnding,
} from "../lib/lineEndings";
import { measure, measureAsync } from "../../../instrumentation";
import type { InstrumentationTags } from "../../../instrumentation";
import type { TemplateFieldValue } from "../../../features/schema-catalog/types";
import type { IndexedDef } from "../../../features/def-index";
import type { CreateDefResult } from "../types/createDef";
import type { UserDefTemplate, UserDefTemplateSummary } from "../types/defTemplates";
import type { XmlEditorMode, XmlEditorSnapshot } from "../types/editorSession";
import type { SavePreview } from "../types/projectSave";
import type {
  ParseDiagnostic,
  ValidationDiagnostic,
  XmlEdit,
  XmlEditContext,
  XmlEditorDocumentView,
} from "../types/xmlDocument";

export interface UseXmlEditorSessionReturn {
  projectId: string;
  relativePath: string;
  readOnly: boolean;
  baseRawXml: string;
  currentRawXml: string;
  currentParseDiagnostics: ParseDiagnostic[];
  currentValidationDiagnostics: ValidationDiagnostic[];
  isBufferValid: boolean;
  lastValidSnapshot: XmlEditorSnapshot | null;
  mode: XmlEditorMode;
  dirty: boolean;
  canUndo: boolean;
  canRedo: boolean;
  savePreview: SavePreview | null;
  saveError: string | null;
  saveBusy: boolean;
  loading: boolean;
  loadError: string | null;
  applyFormEdit: (
    edit: XmlEdit,
    editContext?: XmlEditContext,
  ) => Promise<string>;
  applyFormEdits: (
    edits: XmlEdit[],
    editContext?: XmlEditContext,
  ) => Promise<string>;
  insertDefFromTemplate: (
    defType: string,
    templateId: string | null,
    fieldValues: Record<string, TemplateFieldValue>,
  ) => Promise<CreateDefResult>;
  saveSelectedDefAsTemplate: (name: string) => Promise<UserDefTemplate>;
  insertDefFromUserTemplate: (
    templateId: string,
    defName: string,
  ) => Promise<CreateDefResult>;
  insertDefFromIndexedDef: (
    source: IndexedDef,
    defName: string,
  ) => Promise<CreateDefResult>;
  listUserDefTemplates: (defType: string) => Promise<UserDefTemplateSummary[]>;
  deleteUserDefTemplate: (templateId: string) => Promise<void>;
  updateRawXml: (xml: string) => void;
  switchMode: (mode: XmlEditorMode) => void;
  undo: () => void;
  redo: () => void;
  selectDef: (nodeId: number | null) => void;
  requestSavePreview: (
    traceId?: string,
    source?: string,
    startedAt?: number,
  ) => Promise<void>;
  loadFullSavePreview: () => Promise<void>;
  confirmSave: (traceId?: string, source?: string) => Promise<void>;
  clearSavePreview: () => void;
  savePreviewTraceId: string | null;
  savePreviewStartedAt: number | null;
}

interface HistoryState {
  past: XmlEditorSnapshot[];
  present: XmlEditorSnapshot;
  future: XmlEditorSnapshot[];
}

function makeEmptySnapshot(): XmlEditorSnapshot {
  return {
    rawXml: "",
    parsed: null,
    parseDiagnostics: [],
    validationDiagnostics: [],
    selectedDefNodeId: null,
  };
}

export interface XmlEditorFileRef {
  locationId: string;
  locationName?: string;
  sourceKind: "project" | "source";
  readOnly: boolean;
  relativePath: string;
}

export function useXmlEditorSession(
  projectId: string | undefined,
  file: XmlEditorFileRef | undefined,
): UseXmlEditorSessionReturn | null {
  const relativePath = file?.relativePath;
  const readOnly = file?.readOnly ?? false;
  const [history, setHistory] = useState<HistoryState>(() => ({
    past: [],
    present: makeEmptySnapshot(),
    future: [],
  }));
  const [baseRawXml, setBaseRawXml] = useState("");
  const [mode, setMode] = useState<XmlEditorMode>("form");
  const [loading, setLoading] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [savePreview, setSavePreview] = useState<SavePreview | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [saveBusy, setSaveBusy] = useState(false);

  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const preTypingSnapshotRef = useRef<XmlEditorSnapshot | null>(null);
  const latestRawXmlRef = useRef("");
  const pendingFormEditRef = useRef<Promise<string>>(Promise.resolve(""));
  const savePreviewXmlRef = useRef<string | null>(null);
  const savePreviewTokenRef = useRef<string | null>(null);
  const savePreviewTraceIdRef = useRef<string | null>(null);
  const savePreviewSourceRef = useRef<string | null>(null);
  const savePreviewStartedAtRef = useRef<number | null>(null);
  const lineEndingRef = useRef<LineEnding>("lf");
  const modeRef = useRef(mode);
  modeRef.current = mode;

  const clearPreviewState = useCallback(() => {
    savePreviewXmlRef.current = null;
    savePreviewTokenRef.current = null;
    savePreviewTraceIdRef.current = null;
    savePreviewSourceRef.current = null;
    savePreviewStartedAtRef.current = null;
    setSavePreview(null);
    setSaveError(null);
  }, []);

  useEffect(() => {
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, []);

  useEffect(() => {
    if (!projectId || !relativePath) return;

    let cancelled = false;
    setLoading(true);
    setLoadError(null);
    setSavePreview(null);
    setSaveError(null);
    setSaveBusy(false);
    savePreviewXmlRef.current = null;
    savePreviewTokenRef.current = null;

    const loadPromise = readOnly
      ? readLocationXmlEditorDocument(projectId, file!.locationId, relativePath)
      : readProjectXmlEditorDocument(projectId, relativePath);

    loadPromise
      .then((result) => {
        if (cancelled) return;
        const snapshot: XmlEditorSnapshot = {
          rawXml: result.rawXml,
          parsed: result.document,
          parseDiagnostics: result.parseDiagnostics,
          validationDiagnostics: result.validationDiagnostics,
          selectedDefNodeId: result.document?.defs[0]?.nodeId ?? null,
        };
        lineEndingRef.current = detectLineEnding(result.rawXml);
        latestRawXmlRef.current = result.rawXml;
        setBaseRawXml(result.rawXml);
        setHistory({ past: [], present: snapshot, future: [] });
        const profile = result.document?.profile;
        setMode(
          profile === "patch" ||
            profile === "about" ||
            (profile === "defs" && (result.document?.defs.length ?? 0) > 0)
            ? "form"
            : "raw",
        );
      })
      .catch((e: unknown) => {
        if (!cancelled) setLoadError(formatError(e));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
    // Use stable primitive values so the effect doesn't re-fire when the
    // parent re-renders and passes a new file object with identical contents.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [projectId, file?.locationId, file?.relativePath, file?.readOnly]);

  const applyFormEdits = useCallback(
    (edits: XmlEdit[], editContext?: XmlEditContext): Promise<string> => {
      if (readOnly || !projectId || !relativePath || edits.length === 0) {
        return Promise.resolve(latestRawXmlRef.current);
      }

      const run = pendingFormEditRef.current
        .catch(() => latestRawXmlRef.current)
        .then(async () => {
          const currentXml = latestRawXmlRef.current;
          const result =
            edits.length === 1
              ? await applyXmlEditorEdit(
                  projectId,
                  relativePath,
                  currentXml,
                  edits[0],
                  editContext,
                )
              : await applyXmlEditorEdits(
                  projectId,
                  relativePath,
                  currentXml,
                  edits,
                  editContext,
                );

          if (!result.document) {
            throw new Error("Form edit returned no parsed document.");
          }

          const changed = latestRawXmlRef.current !== result.rawXml;
          latestRawXmlRef.current = result.rawXml;
          if (changed) clearPreviewState();

          // Commit the post-commit history update synchronously. The form controller now
          // skips the redundant whole-form rebuild for a form-originated commit (Step 4),
          // so there is no longer an expensive render to defer here - the earlier
          // startTransition only added a small delay and is removed.
          setHistory((prev) => {
            const next: XmlEditorSnapshot = {
              rawXml: result.rawXml,
              parsed: result.document,
              parseDiagnostics: result.parseDiagnostics,
              validationDiagnostics: result.validationDiagnostics,
              selectedDefNodeId: prev.present.selectedDefNodeId,
            };
            if (prev.present.rawXml === next.rawXml) {
              return { ...prev, present: next };
            }
            return {
              past: [...prev.past, prev.present],
              present: next,
              future: [],
            };
          });

          return result.rawXml;
        })
        .catch((e: unknown) => {
          throw new Error(formatError(e));
        });

      pendingFormEditRef.current = run;
      return run;
    },
    [projectId, relativePath, clearPreviewState],
  );

  const applyFormEdit = useCallback(
    (edit: XmlEdit, editContext?: XmlEditContext): Promise<string> =>
      applyFormEdits([edit], editContext),
    [applyFormEdits],
  );

  const flushPendingFormEdits = useCallback(async () => {
    await pendingFormEditRef.current.catch(() => latestRawXmlRef.current);
    return latestRawXmlRef.current;
  }, []);

  const updateRawXml = useCallback(
    (xml: string) => {
      if (readOnly) return;
      latestRawXmlRef.current = xml;
      clearPreviewState();

      setHistory((prev) => {
        if (!preTypingSnapshotRef.current) {
          preTypingSnapshotRef.current = prev.present;
        }
        return {
          ...prev,
          present: {
            ...prev.present,
            rawXml: xml,
            parsed: null,
            validationDiagnostics: [],
          },
        };
      });

      if (debounceRef.current) clearTimeout(debounceRef.current);
      debounceRef.current = setTimeout(() => {
        if (!projectId || !relativePath) return;
        const preTyping = preTypingSnapshotRef.current;
        preTypingSnapshotRef.current = null;

        parseXmlEditorBuffer(projectId, relativePath, xml)
          .then((result) => {
            setHistory((prev) => {
              if (prev.present.rawXml !== xml) return prev;
              const next: XmlEditorSnapshot = {
                rawXml: xml,
                parsed: result.document,
                parseDiagnostics: result.parseDiagnostics,
                validationDiagnostics: result.validationDiagnostics,
                selectedDefNodeId: prev.present.selectedDefNodeId,
              };
              if (preTyping && preTyping.rawXml !== xml) {
                return {
                  past: [...prev.past, preTyping],
                  present: next,
                  future: [],
                };
              }
              return { ...prev, present: next, future: [] };
            });
          })
          .catch(() => {
            setHistory((prev) => {
              if (prev.present.rawXml !== xml) return prev;
              const next: XmlEditorSnapshot = {
                ...prev.present,
                parsed: null,
                validationDiagnostics: [],
              };
              if (preTyping && preTyping.rawXml !== xml) {
                return {
                  past: [...prev.past, preTyping],
                  present: next,
                  future: [],
                };
              }
              return { ...prev, present: next };
            });
          });
      }, 300);
    },
    [projectId, relativePath, clearPreviewState],
  );

  const switchMode = useCallback(
    (next: XmlEditorMode) => {
      if (next === "form" && !history.present.parsed) return;
      setMode(next);
    },
    [history.present.parsed],
  );

  const undo = useCallback(() => {
    setHistory((prev) => {
      if (prev.past.length === 0) return prev;
      const past = [...prev.past];
      const previous = past.pop()!;
      latestRawXmlRef.current = previous.rawXml;
      clearPreviewState();
      return {
        past,
        present: previous,
        future: [prev.present, ...prev.future],
      };
    });
  }, [clearPreviewState]);

  const redo = useCallback(() => {
    setHistory((prev) => {
      if (prev.future.length === 0) return prev;
      const [next, ...rest] = prev.future;
      latestRawXmlRef.current = next.rawXml;
      clearPreviewState();
      return {
        past: [...prev.past, prev.present],
        present: next,
        future: rest,
      };
    });
  }, [clearPreviewState]);

  const selectDef = useCallback((nodeId: number | null) => {
    setHistory((prev) => ({
      ...prev,
      present: { ...prev.present, selectedDefNodeId: nodeId },
    }));
  }, []);

  const requestSavePreview = useCallback(
    async (traceId?: string, source?: string, startedAt?: number) => {
      if (readOnly || !projectId || !relativePath) return;
      setSaveBusy(true);
      setSaveError(null);
      savePreviewTraceIdRef.current = traceId ?? null;
      savePreviewSourceRef.current = source ?? null;
      // Prefer the timestamp captured at the click boundary so dialogRendered
      // includes form-flush time; fall back to now only if no caller provided one.
      savePreviewStartedAtRef.current = startedAt ?? performance.now();
      const baseTags: InstrumentationTags = {
        relativePath,
        mode: modeRef.current,
        ...(traceId != null ? { traceId } : {}),
        ...(source != null ? { source } : {}),
      };
      try {
        const flushed = await measureAsync(
          "xmlEditor.previewSave.flushPendingFormEdits",
          flushPendingFormEdits,
          baseTags,
        );
        const proposedXml = measure(
          "xmlEditor.previewSave.applyLineEnding",
          () => applyLineEnding(flushed, lineEndingRef.current),
          baseTags,
        );
        const preview = await measureAsync(
          "xmlEditor.previewSave.invokePreview",
          () =>
            previewProjectXmlSave(
              projectId,
              relativePath,
              proposedXml,
              traceId,
            ),
          baseTags,
        );
        measure(
          "xmlEditor.previewSave.setPreviewState",
          () => {
            savePreviewXmlRef.current = proposedXml;
            savePreviewTokenRef.current = preview.validationToken;
            setSavePreview(preview);
          },
          baseTags,
        );
      } catch (e: unknown) {
        setSaveError(formatError(e));
      } finally {
        setSaveBusy(false);
      }
    },
    [projectId, relativePath, flushPendingFormEdits],
  );

  // Re-fetch the current preview with the full (uncollapsed) diff. Reuses the proposed XML
  // captured when the preview opened, so it doesn't re-flush the form - only the diff
  // presentation changes. Refreshes the validation token too, since confirm relies on it.
  const loadFullSavePreview = useCallback(async () => {
    if (readOnly || !projectId || !relativePath) return;
    const proposedXml = savePreviewXmlRef.current;
    if (!proposedXml) return;
    setSaveBusy(true);
    setSaveError(null);
    try {
      const preview = await previewProjectXmlSave(
        projectId,
        relativePath,
        proposedXml,
        savePreviewTraceIdRef.current ?? undefined,
        true,
      );
      savePreviewTokenRef.current = preview.validationToken;
      setSavePreview(preview);
    } catch (e: unknown) {
      setSaveError(formatError(e));
    } finally {
      setSaveBusy(false);
    }
  }, [projectId, relativePath, readOnly]);

  const confirmSave = useCallback(
    async (traceId?: string, source?: string) => {
      if (readOnly || !projectId || !relativePath) return;
      const proposedXml = savePreviewXmlRef.current;
      if (!proposedXml) return;

      const effectiveTraceId =
        traceId ?? savePreviewTraceIdRef.current ?? undefined;
      // Tag the confirm with its own source ("dialog"), falling back to the
      // source that opened the preview when the caller didn't pass one.
      const effectiveSource =
        source ?? savePreviewSourceRef.current ?? undefined;
      const baseTags: InstrumentationTags = {
        relativePath,
        phase: "confirm",
        ...(effectiveTraceId != null ? { traceId: effectiveTraceId } : {}),
        ...(effectiveSource != null ? { source: effectiveSource } : {}),
      };

      setSaveBusy(true);
      setSaveError(null);
      try {
        await measureAsync(
          "xmlEditor.previewSave.confirm.total",
          async () => {
            await measureAsync(
              "xmlEditor.previewSave.confirm.invokeSave",
              () =>
                saveProjectXmlFile(
                  projectId,
                  relativePath,
                  proposedXml,
                  savePreviewTokenRef.current ?? undefined,
                  effectiveTraceId,
                ),
              baseTags,
            );
            latestRawXmlRef.current = proposedXml;
            setBaseRawXml(proposedXml);
            savePreviewXmlRef.current = null;
            savePreviewTokenRef.current = null;
            setSavePreview(null);

            const result = await measureAsync(
              "xmlEditor.previewSave.confirm.postSaveParse",
              () => parseXmlEditorBuffer(projectId, relativePath, proposedXml),
              baseTags,
            );
            measure(
              "xmlEditor.previewSave.confirm.setHistory",
              () => {
                setHistory((prev) => ({
                  ...prev,
                  present: {
                    rawXml: proposedXml,
                    parsed: result.document,
                    parseDiagnostics: result.parseDiagnostics,
                    validationDiagnostics: result.validationDiagnostics,
                    selectedDefNodeId: prev.present.selectedDefNodeId,
                  },
                }));
              },
              baseTags,
            );
          },
          baseTags,
        );
      } catch (e: unknown) {
        setSaveError(formatError(e));
      } finally {
        setSaveBusy(false);
      }
    },
    [projectId, relativePath],
  );

  const clearSavePreview = useCallback(() => {
    clearPreviewState();
  }, [clearPreviewState]);

  const insertDefFromTemplate = useCallback(
    async (
      defType: string,
      templateId: string | null,
      fieldValues: Record<string, TemplateFieldValue>,
    ): Promise<CreateDefResult> => {
      if (readOnly || !projectId || !relativePath) {
        throw new Error("Cannot insert def: read-only or no active file.");
      }
      await pendingFormEditRef.current.catch(() => latestRawXmlRef.current);
      const currentXml = latestRawXmlRef.current;

      const result = await createDefFromTemplate(
        projectId,
        relativePath,
        currentXml,
        defType,
        templateId,
        fieldValues,
      );

      const { editorDocument } = result;
      latestRawXmlRef.current = editorDocument.rawXml;
      clearPreviewState();

      setHistory((prev) => {
        const next: XmlEditorSnapshot = {
          rawXml: editorDocument.rawXml,
          parsed: editorDocument.document,
          parseDiagnostics: editorDocument.parseDiagnostics,
          validationDiagnostics: editorDocument.validationDiagnostics,
          selectedDefNodeId: result.insertedNodeId,
        };
        return {
          past: [...prev.past, prev.present],
          present: next,
          future: [],
        };
      });

      return result;
    },
    [projectId, relativePath, readOnly, clearPreviewState],
  );

  // Like insertDefFromTemplate, this only awaits edits already dispatched to
  // the backend (pendingFormEditRef) - it does not itself commit in-progress
  // form-field-store edits into the buffer. Callers must flush form drafts
  // (formApi.flushAll(), via XmlEditorPane's flushFormDrafts()) before calling
  // this, exactly as XmlEditorPane already does for insertDefFromTemplate.
  const insertDefFromUserTemplate = useCallback(
    async (templateId: string, defName: string): Promise<CreateDefResult> => {
      if (readOnly || !projectId || !relativePath) {
        throw new Error("Cannot insert def: read-only or no active file.");
      }
      await pendingFormEditRef.current.catch(() => latestRawXmlRef.current);
      const currentXml = latestRawXmlRef.current;

      const result = await createDefFromUserTemplate(
        projectId,
        relativePath,
        currentXml,
        templateId,
        defName,
      );

      const { editorDocument } = result;
      latestRawXmlRef.current = editorDocument.rawXml;
      clearPreviewState();

      setHistory((prev) => {
        const next: XmlEditorSnapshot = {
          rawXml: editorDocument.rawXml,
          parsed: editorDocument.document,
          parseDiagnostics: editorDocument.parseDiagnostics,
          validationDiagnostics: editorDocument.validationDiagnostics,
          selectedDefNodeId: result.insertedNodeId,
        };
        return {
          past: [...prev.past, prev.present],
          present: next,
          future: [],
        };
      });

      return result;
    },
    [projectId, relativePath, readOnly, clearPreviewState],
  );

  // Like insertDefFromUserTemplate, this only awaits edits already dispatched to
  // the backend (pendingFormEditRef) - callers must flush form drafts first via
  // XmlEditorPane's flushFormDrafts() wrapper.
  const insertDefFromIndexedDef = useCallback(
    async (source: IndexedDef, defName: string): Promise<CreateDefResult> => {
      if (readOnly || !projectId || !relativePath) {
        throw new Error("Cannot insert def: read-only or no active file.");
      }
      await pendingFormEditRef.current.catch(() => latestRawXmlRef.current);
      const currentXml = latestRawXmlRef.current;

      const result = await createDefFromIndexedDef(
        projectId,
        relativePath,
        currentXml,
        source.source.locationId,
        source.relativePath,
        source.defType,
        source.defName,
        source.nodeId ?? null,
        defName,
      );

      const { editorDocument } = result;
      latestRawXmlRef.current = editorDocument.rawXml;
      clearPreviewState();

      setHistory((prev) => {
        const next: XmlEditorSnapshot = {
          rawXml: editorDocument.rawXml,
          parsed: editorDocument.document,
          parseDiagnostics: editorDocument.parseDiagnostics,
          validationDiagnostics: editorDocument.validationDiagnostics,
          selectedDefNodeId: result.insertedNodeId,
        };
        return {
          past: [...prev.past, prev.present],
          present: next,
          future: [],
        };
      });

      return result;
    },
    [projectId, relativePath, readOnly, clearPreviewState],
  );

  // Persists the currently selected Def to app storage. Unlike insertDefFromTemplate,
  // this does not touch currentRawXml/history/baseRawXml - saving a template changes app
  // storage only, so the editor buffer and dirty state must stay exactly as they were.
  const saveSelectedDefAsTemplate = useCallback(
    async (name: string): Promise<UserDefTemplate> => {
      if (readOnly || !projectId || !relativePath) {
        throw new Error("Cannot save template: read-only or no active file.");
      }
      await pendingFormEditRef.current.catch(() => latestRawXmlRef.current);
      const nodeId = history.present.selectedDefNodeId;
      if (nodeId == null) {
        throw new Error("No Def is selected.");
      }
      const currentXml = latestRawXmlRef.current;
      return saveUserDefTemplate(projectId, relativePath, currentXml, nodeId, name);
    },
    [projectId, relativePath, readOnly, history.present.selectedDefNodeId],
  );

  const listUserDefTemplates = useCallback(
    (defType: string): Promise<UserDefTemplateSummary[]> => {
      if (!projectId) return Promise.resolve([]);
      return listUserDefTemplatesApi(projectId, defType);
    },
    [projectId],
  );

  // Deletes app storage only, like saveSelectedDefAsTemplate - never touches
  // currentRawXml/history/baseRawXml, so it must not mark the editor dirty.
  const deleteUserDefTemplate = useCallback(
    async (templateId: string): Promise<void> => {
      if (!projectId) {
        throw new Error("Cannot delete template: no active project.");
      }
      await deleteUserDefTemplateApi(projectId, templateId);
    },
    [projectId],
  );

  const present = history.present;
  const serializedPresent = applyLineEnding(
    present.rawXml,
    lineEndingRef.current,
  );
  const dirty = readOnly ? false : serializedPresent !== baseRawXml;
  const lastValidSnapshot = deriveLastValidSnapshot(
    history,
    preTypingSnapshotRef.current,
  );

  if (!projectId || !relativePath) return null;

  return {
    projectId,
    relativePath,
    readOnly,
    baseRawXml,
    currentRawXml: present.rawXml,
    currentParseDiagnostics: lastValidSnapshot?.parseDiagnostics ?? present.parseDiagnostics,
    currentValidationDiagnostics:
      lastValidSnapshot?.validationDiagnostics ?? present.validationDiagnostics,
    isBufferValid: lastValidSnapshot?.parsed != null,
    lastValidSnapshot,
    mode,
    dirty,
    canUndo: history.past.length > 0,
    canRedo: history.future.length > 0,
    savePreview,
    saveError,
    saveBusy,
    loading,
    loadError,
    applyFormEdit,
    applyFormEdits,
    insertDefFromTemplate,
    insertDefFromUserTemplate,
    insertDefFromIndexedDef,
    saveSelectedDefAsTemplate,
    listUserDefTemplates,
    deleteUserDefTemplate,
    updateRawXml,
    switchMode,
    undo,
    redo,
    selectDef,
    requestSavePreview,
    loadFullSavePreview,
    confirmSave,
    clearSavePreview,
    savePreviewTraceId: savePreviewTraceIdRef.current,
    savePreviewStartedAt: savePreviewStartedAtRef.current,
  };
}

function deriveLastValidSnapshot(
  history: HistoryState,
  preTyping: XmlEditorSnapshot | null,
): XmlEditorSnapshot | null {
  if (history.present.parsed) return history.present;
  for (let i = history.past.length - 1; i >= 0; i--) {
    if (history.past[i].parsed) return history.past[i];
  }
  if (preTyping?.parsed) return preTyping;
  return null;
}

export type { XmlEditorMode, XmlEditorSnapshot };
export type { XmlEditorDocumentView, ParseDiagnostic };
