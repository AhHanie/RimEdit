import { useCallback, useEffect, useRef, useState } from "react";
import { measureAsync, generateTraceId } from "../../../../instrumentation";
import type { InstrumentationTags } from "../../../../instrumentation";
import { ChevronRight, FileCode2, Loader2 } from "lucide-react";
import type { ActiveEditorCommands } from "../../../editor-workspace/types";
import type { SchemaCatalog } from "../../../schema-catalog";
import { PatchEditorPane, PatchPreviewDialog } from "../../../patches-editor";
import type { PatchPreviewTarget } from "../../../patches-editor";
import { AboutEditorPane } from "../../../about-editor";
import type { XmlEditorMode } from "../../types/editorSession";
import type { TemplateFieldValue } from "../../types/createDef";
import type { IndexedDef } from "../../../def-index";
import { useXmlEditorSession, type XmlEditorFileRef } from "../../hooks/useXmlEditorSession";
import { useXmlFormController } from "../../hooks/useXmlFormController";
import { useFormViews } from "../../../form-views/hooks/useFormViews";
import { FORM_VIEW_SELECTOR_SELECT_ID } from "../../../form-views/components/FormViewSelector/FormViewSelector";
import { XmlEditorContextProvider } from "../../context/XmlEditorContext";
import { XmlEditorToolbar } from "../XmlEditorToolbar/XmlEditorToolbar";
import { XmlFormEditor } from "../XmlFormEditor/XmlFormEditor";
import { XmlRawEditor } from "../XmlRawEditor/XmlRawEditor";
import { XmlDiagnosticsPanel } from "../XmlDiagnosticsPanel/XmlDiagnosticsPanel";
import { SavePreviewDialog } from "../SavePreviewDialog/SavePreviewDialog";
import { CreateDefWizard } from "../CreateDefWizard/CreateDefWizard";
import { SaveDefTemplateDialog } from "../SaveDefTemplateDialog/SaveDefTemplateDialog";
import styles from "./XmlEditorPane.module.css";

interface Props {
  projectId: string | undefined;
  file: XmlEditorFileRef | undefined;
  catalog: SchemaCatalog | null;
  /** Form Views (issue 06) are scoped by `{project, gameVersion, defType}` -- threaded down from
   * `ProjectSettings.gameVersion` (`AppShell` -> `EditorWorkspace` -> here) so `useFormViews` can
   * resolve/persist selections against the right custom-view scope. */
  gameVersion?: string;
  hasOpenTabs: boolean;
  active?: boolean;
  selectedDefNodeId?: number;
  selectionRequestId?: number;
  createDefSignal?: number;
  onDirtyChange?: (dirty: boolean) => void;
  onNavigateDef?: (fileRef: XmlEditorFileRef, nodeId: number | null) => void;
  onCloseActiveTab?: () => Promise<void>;
  onActiveCommandsChange?: (commands: ActiveEditorCommands | null) => void;
}

export function XmlEditorPane({
  projectId,
  file,
  catalog,
  gameVersion,
  hasOpenTabs,
  active,
  selectedDefNodeId,
  selectionRequestId,
  createDefSignal,
  onDirtyChange,
  onNavigateDef,
  onCloseActiveTab,
  onActiveCommandsChange,
}: Props) {
  const [wizardOpen, setWizardOpen] = useState(false);
  const [saveTemplateOpen, setSaveTemplateOpen] = useState(false);
  const [previewOpen, setPreviewOpen] = useState(false);
  const patchFlushRef = useRef<() => Promise<void>>(async () => undefined);
  const aboutFlushRef = useRef<() => Promise<void>>(async () => undefined);
  const session = useXmlEditorSession(projectId, file);
  const editorSnapshot = session
    ? session.lastValidSnapshot ?? {
        rawXml: session.currentRawXml,
        parsed: null,
        parseDiagnostics: [],
        validationDiagnostics: [],
        selectedDefNodeId: null,
      }
    : null;
  // Hoisted early (before any conditional return) because `useFormViews` must be called
  // unconditionally like every other hook here. Reused below (unchanged) for the
  // save-as-template/patch-preview-target computations that used to duplicate this lookup.
  const selectedDefNodeIdEarly = session?.lastValidSnapshot?.selectedDefNodeId ?? null;
  const selectedDefEarly =
    editorSnapshot?.parsed?.defs.find((d) => d.nodeId === selectedDefNodeIdEarly) ?? null;
  const selectedDefOrdinalEarly = selectedDefEarly
    ? (editorSnapshot?.parsed?.defs.findIndex((d) => d.nodeId === selectedDefEarly.nodeId) ?? -1)
    : -1;
  const documentProfileEarly = editorSnapshot?.parsed?.profile;

  const formViews = useFormViews({
    projectId: projectId ?? null,
    gameVersion,
    catalog,
    pane: file
      ? { locationId: file.locationId, relativePath: file.relativePath, sourceKind: file.sourceKind }
      : null,
    selectedDef:
      documentProfileEarly === "defs" && selectedDefEarly && selectedDefOrdinalEarly >= 0
        ? { defType: selectedDefEarly.defType, ordinal: selectedDefOrdinalEarly }
        : null,
  });

  // Form Views (issue 05's `onFocusedFieldHidden` signal, wired up by issue 06): when a view
  // switch hides the top-level root of the field the user was actually focused in (real DOM
  // focus, not a click-triggered blur -- see the doc comment on `useXmlFormController`), that
  // control unmounts and focus would otherwise fall back to `document.body`/nowhere. Redirect it
  // to the Form View selector's `<select>` -- always present whenever Form View controls are
  // applicable at all -- so the user lands somewhere meaningful instead of losing focus outright
  // (Plan.md section 7: "restore focus to the selector/customize control if the focused field is
  // removed"). A plain DOM id lookup (rather than threading a ref through `XmlFormEditor` into
  // `FormViewSelector`) keeps this a one-line, low-coupling fix; `FormViewSelector` already
  // exports this id for exactly this purpose.
  const onFocusedFieldHidden = useCallback(() => {
    document.getElementById(FORM_VIEW_SELECTOR_SELECT_ID)?.focus();
  }, []);

  const formApi = useXmlFormController({
    snapshot: editorSnapshot,
    catalog,
    selectedDefNodeId: session?.lastValidSnapshot?.selectedDefNodeId ?? null,
    commitEdits: session?.applyFormEdits ?? (() => Promise.resolve("")),
    clearPreview: session?.clearSavePreview ?? (() => undefined),
    visibleTopLevelFieldIds: formViews.visibleTopLevelFieldIds,
    onFocusedFieldHidden,
  });
  const effectiveDirty =
    !!session && !session.readOnly && (session.dirty || formApi.hasDraftChanges || formApi.hasPendingCommits);

  useEffect(() => {
    onDirtyChange?.(effectiveDirty);
  }, [onDirtyChange, effectiveDirty]);

  // Refs so command closures always call the latest session/formApi/close handler.
  const sessionRef = useRef(session);
  sessionRef.current = session;
  const formApiRef = useRef(formApi);
  formApiRef.current = formApi;
  const onCloseActiveTabRef = useRef(onCloseActiveTab);
  onCloseActiveTabRef.current = onCloseActiveTab;

  // Stable def-selection handler so the memoized XmlFormEditor isn't re-rendered just
  // because the parent re-rendered (e.g. on a save-state toggle). Reads latest session/
  // formApi via refs and flushes pending form drafts before switching defs.
  const handleSelectDef = useCallback(async (nodeId: number | null) => {
    const s = sessionRef.current;
    const fa = formApiRef.current;
    if (!s) return;
    try {
      if (s.mode === "form") await fa.flushAll();
    } catch {
      // Field/form errors are recorded by the form controller.
      return;
    }
    s.selectDef(nodeId);
  }, []);

  // Scalar guard values for the command-publishing effect deps.
  const readOnly = file?.readOnly ?? false;
  const commandHasBlockingValidation =
    session?.currentValidationDiagnostics.some((d) => d.blocking) ?? false;
  const commandIsBufferValid =
    (session?.isBufferValid ?? false) &&
    !commandHasBlockingValidation &&
    !(session?.mode === "form" && formApi.hasBlockingErrors);
  const commandCanUndo =
    (session?.canUndo ?? false) &&
    !formApi.hasDraftChanges &&
    !formApi.hasPendingCommits &&
    !readOnly;
  const commandCanRedo =
    (session?.canRedo ?? false) &&
    !formApi.hasDraftChanges &&
    !formApi.hasPendingCommits &&
    !readOnly;
  const commandCanSave =
    !readOnly &&
    commandIsBufferValid &&
    !!(session?.currentRawXml) &&
    !(session?.saveBusy ?? false);
  const sessionReady = !!session && !session.loading && !session.loadError;

  // Publish active editor commands when this pane is active and the session is ready.
  // Inactive panes set up a cleanup-only path so the cleanup (publishing null) fires
  // before the newly active pane's effect runs, avoiding a race between sibling panes.
  useEffect(() => {
    if (!active) {
      return () => onActiveCommandsChange?.(null);
    }
    if (!sessionReady) {
      onActiveCommandsChange?.(null);
      return () => onActiveCommandsChange?.(null);
    }
    onActiveCommandsChange?.({
      undo: () => { if (commandCanUndo) sessionRef.current?.undo(); },
      redo: () => { if (commandCanRedo) sessionRef.current?.redo(); },
      save: async () => {
        const s = sessionRef.current;
        const fa = formApiRef.current;
        if (!s || s.readOnly || !s.isBufferValid || !s.currentRawXml || s.saveBusy) return;
        const startedAt = performance.now();
        const traceId = generateTraceId();
        const entryTags: InstrumentationTags = { traceId, relativePath: s.relativePath, source: "commandPalette", mode: s.mode };
        try {
          await measureAsync("xmlEditor.previewSave.entrypoint", async () => {
            if (s.mode === "form") {
              const profile = s.lastValidSnapshot?.parsed?.profile;
              const flush =
                profile === "patch"
                  ? patchFlushRef.current
                  : profile === "about"
                    ? aboutFlushRef.current
                    : async () => {
                        await fa.flushAll();
                      };
              await measureAsync("xmlEditor.previewSave.formFlushAll", flush, entryTags);
            }
            await s.requestSavePreview(traceId, "commandPalette", startedAt);
          }, entryTags);
        } catch { /* form controller records field errors */ }
      },
      close: async () => { await onCloseActiveTabRef.current?.(); },
      canUndo: commandCanUndo,
      canRedo: commandCanRedo,
      canSave: commandCanSave,
      canClose: true,
    });
    return () => onActiveCommandsChange?.(null);
  }, [active, sessionReady, commandCanUndo, commandCanRedo, commandCanSave, onActiveCommandsChange]);

  // Open wizard from the command palette signal.
  useEffect(() => {
    if (createDefSignal && !file?.readOnly && catalog && session && !session.loading) {
      setWizardOpen(true);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [createDefSignal]);

  useEffect(() => {
    if (
      selectionRequestId !== undefined &&
      selectedDefNodeId !== undefined &&
      session &&
      !session.loading &&
      !session.loadError
    ) {
      session.selectDef(selectedDefNodeId);
    }
    // Fires on both selectionRequestId change and loading completion so newly
    // opened tabs select the target def once the file finishes loading.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectionRequestId, session?.loading]);

  if (!projectId || !file || !session) {
    return (
      <div className={`${styles.root} ${styles.rootEmpty}`}>
        <div className="state-empty">
          {!hasOpenTabs && <FileCode2 size={32} className="state-empty-icon" />}
          <p className="state-empty-text">
            {hasOpenTabs ? "Select a file to view its contents." : "Open a file from the explorer."}
          </p>
        </div>
      </div>
    );
  }

  const activeSession = session;
  const activeEditorSnapshot = editorSnapshot!;
  const documentProfile = activeEditorSnapshot.parsed?.profile;
  const isPatchFile = documentProfile === "patch";
  const isAboutFile = documentProfile === "about";
  const hasBlockingValidationErrors = activeSession.currentValidationDiagnostics.some(
    (diagnostic) => diagnostic.blocking,
  );

  async function flushFormDrafts(timingTags?: InstrumentationTags) {
    if (activeSession.mode !== "form") return;
    const flush = isPatchFile
      ? patchFlushRef.current
      : isAboutFile
        ? aboutFlushRef.current
        : async () => {
            await formApi.flushAll();
          };
    if (timingTags) {
      await measureAsync("xmlEditor.previewSave.formFlushAll", flush, timingTags);
    } else {
      await flush();
    }
  }

  const editorSession = {
    ...activeSession,
    dirty: effectiveDirty,
    isBufferValid:
      activeSession.isBufferValid &&
      !hasBlockingValidationErrors &&
      !(activeSession.mode === "form" && formApi.hasBlockingErrors),
    canUndo: activeSession.canUndo && !formApi.hasDraftChanges && !formApi.hasPendingCommits,
    canRedo: activeSession.canRedo && !formApi.hasDraftChanges && !formApi.hasPendingCommits,
    insertDefFromTemplate: async (defType: string, templateId: string | null, fieldValues: Record<string, TemplateFieldValue>) => {
      await flushFormDrafts();
      return activeSession.insertDefFromTemplate(defType, templateId, fieldValues);
    },
    insertDefFromUserTemplate: async (templateId: string, defName: string) => {
      await flushFormDrafts();
      return activeSession.insertDefFromUserTemplate(templateId, defName);
    },
    insertDefFromIndexedDef: async (source: IndexedDef, defName: string) => {
      await flushFormDrafts();
      return activeSession.insertDefFromIndexedDef(source, defName);
    },
    saveSelectedDefAsTemplate: async (name: string) => {
      await flushFormDrafts();
      return activeSession.saveSelectedDefAsTemplate(name);
    },
    switchMode: (next: XmlEditorMode) => {
      if (next === activeSession.mode) return;
      if (activeSession.mode === "form") {
        void flushFormDrafts()
          .then(() => activeSession.switchMode(next))
          .catch(() => undefined);
        return;
      }
      activeSession.switchMode(next);
    },
    selectDef: (nodeId: number | null) => {
      void flushFormDrafts()
        .then(() => activeSession.selectDef(nodeId))
        .catch(() => undefined);
    },
    requestSavePreview: async (traceId?: string, source?: string, startedAt?: number) => {
      const tags: InstrumentationTags = {
        relativePath,
        mode: activeSession.mode,
        ...(traceId != null ? { traceId } : {}),
        ...(source != null ? { source } : {}),
      };
      try {
        await measureAsync("xmlEditor.previewSave.flushFormDrafts", () => flushFormDrafts(tags), tags);
        await measureAsync("xmlEditor.previewSave.sessionRequest", () => activeSession.requestSavePreview(traceId, source, startedAt), tags);
      } catch {
        // The form controller has already recorded the field/form error.
      }
    },
  };

  if (activeSession.loading) {
    return (
      <div className={`${styles.root} ${styles.rootEmpty}`}>
        <div className="state-loading">
          <Loader2 size={14} className="spin" />
          <span>Loading…</span>
        </div>
      </div>
    );
  }

  if (activeSession.loadError) {
    return (
      <div className={`${styles.root} ${styles.rootEmpty}`}>
        <div className="state-empty">
          <p className="state-empty-text" style={{ color: "var(--text-error)" }}>
            {activeSession.loadError}
          </p>
        </div>
      </div>
    );
  }

  const relativePath = file.relativePath;
  const segments = relativePath.split("/");
  const diagnostics = [
    ...activeSession.currentParseDiagnostics,
    ...activeSession.currentValidationDiagnostics,
  ];

  // Reuses the hoisted-early lookup (computed before the loading/error early returns above,
  // where every hook call including `useFormViews` must happen) instead of recomputing the same
  // find/findIndex over `activeEditorSnapshot.parsed.defs` a second time.
  const selectedDefForTemplate = selectedDefEarly;
  const canSaveAsTemplate =
    !file.readOnly &&
    editorSession.isBufferValid &&
    selectedDefForTemplate != null;
  const defaultTemplateName = selectedDefForTemplate
    ? selectedDefForTemplate.label ||
      selectedDefForTemplate.defName ||
      `${selectedDefForTemplate.defType} template`
    : "New template";

  // Abstract/template Defs (e.g. `<ThingDef Name="BaseThing" Abstract="True">`) have no
  // `defName` -- the preview engine verifies this identity against the resolved element as
  // validation data (see `PatchPreviewTarget`'s doc comment on the backend), so `Name` is the
  // other stable identity to fall back to.
  const selectedDefIdentityForPreview =
    selectedDefForTemplate?.defName ??
    selectedDefForTemplate?.attributes.find((a) => a.name === "Name")?.value ??
    null;
  // The Def's zero-based position among this file's own top-level Defs -- matches the ordinal
  // `xml_document::def_summary::extract_def_summaries` assigns on the backend for the same file,
  // so the preview engine can resolve this exact opened Def by file origin + ordinal instead of a
  // same-named lookup across every registered location (see `PatchPreviewTarget`). Reuses the
  // hoisted-early computation (also fed to `useFormViews`) rather than recomputing it.
  const selectedDefOrdinalForPreview = selectedDefOrdinalEarly;
  const previewTarget: PatchPreviewTarget | null =
    selectedDefForTemplate != null &&
    selectedDefIdentityForPreview != null &&
    selectedDefOrdinalForPreview >= 0
      ? {
          locationId: file.locationId,
          relativePath: file.relativePath,
          defType: selectedDefForTemplate.defType,
          identity: selectedDefIdentityForPreview,
          ordinal: selectedDefOrdinalForPreview,
        }
      : null;
  const canPreviewPatches = previewTarget != null;

  return (
    <div className={styles.root} style={{ position: "relative" }}>
      {/* Breadcrumb */}
      <div className={styles.breadcrumb}>
        {segments.map((seg, i) => (
          <span key={i} className={styles.breadcrumbSegment}>
            {i > 0 && <ChevronRight size={10} className={styles.breadcrumbSep} />}
            {seg}
          </span>
        ))}
        {file.readOnly && file.locationName && (
          <span className={styles.breadcrumbSegment} style={{ marginLeft: "auto", opacity: 0.6, fontSize: "0.8em" }}>
            Read-only · {file.locationName}
          </span>
        )}
      </div>

      {/* Toolbar */}
      <XmlEditorToolbar
        session={editorSession}
        onCreateDef={() => setWizardOpen(true)}
        canCreateDef={
          !file.readOnly && catalog != null && activeSession.isBufferValid && !isPatchFile && !isAboutFile
        }
        onSaveAsTemplate={() => setSaveTemplateOpen(true)}
        canSaveAsTemplate={canSaveAsTemplate}
        onPreviewPatches={() => setPreviewOpen(true)}
        canPreviewPatches={canPreviewPatches}
      />

      {/* Editor body */}
      <div className={styles.body}>
        {activeSession.mode === "form" && isPatchFile ? (
          <PatchEditorPane
            relativePath={relativePath}
            rawXml={activeSession.currentRawXml}
            readOnly={file.readOnly}
            catalog={catalog}
            projectId={projectId}
            onChangeRawXml={activeSession.updateRawXml}
            registerFlush={(flush) => {
              patchFlushRef.current = flush;
            }}
          />
        ) : activeSession.mode === "form" && isAboutFile ? (
          activeEditorSnapshot.parsed?.about ? (
            <AboutEditorPane
              about={activeEditorSnapshot.parsed.about}
              diagnostics={activeSession.currentValidationDiagnostics}
              readOnly={file.readOnly}
              locationName={file.locationName}
              applyFormEdit={activeSession.applyFormEdit}
              registerFlush={(flush) => {
                aboutFlushRef.current = flush;
              }}
            />
          ) : (
            <div className="state-empty">
              <p className="state-empty-text">
                About.xml root element must be &lt;ModMetaData&gt;. Switch to XML to fix it.
              </p>
            </div>
          )
        ) : activeSession.mode === "form" ? (
          <XmlEditorContextProvider value={{ projectId, readOnly: file.readOnly, catalog, onNavigateDef }}>
            <XmlFormEditor
              snapshot={activeEditorSnapshot}
              selectedDefNodeId={activeSession.lastValidSnapshot?.selectedDefNodeId ?? null}
              onSelectDef={handleSelectDef}
              formApi={formApi}
              formViews={formViews}
            />
          </XmlEditorContextProvider>
        ) : (
          <XmlRawEditor
            value={activeSession.currentRawXml}
            onChange={activeSession.updateRawXml}
            readOnly={file.readOnly}
            onShortcut={(shortcut) => {
              if (shortcut === "undo") {
                if (editorSession.canUndo) editorSession.undo();
                return true;
              }
              if (shortcut === "redo") {
                if (editorSession.canRedo) editorSession.redo();
                return true;
              }
              if (shortcut === "save") {
                if (commandCanSave) {
                  const startedAt = performance.now();
                  const traceId = generateTraceId();
                  void measureAsync(
                    "xmlEditor.previewSave.entrypoint",
                    () => editorSession.requestSavePreview(traceId, "shortcut", startedAt),
                    { traceId, relativePath: file.relativePath, source: "shortcut", mode: "raw" },
                  );
                }
                return true;
              }
              if (shortcut === "close") {
                void onCloseActiveTab?.();
                return true;
              }
            }}
          />
        )}
      </div>

      {/* Diagnostics */}
      <XmlDiagnosticsPanel diagnostics={diagnostics} />

      {/* Save preview modal */}
      {activeSession.savePreview && <SavePreviewDialog session={editorSession} />}

      {/* Create Def wizard */}
      {wizardOpen && catalog && (
        <CreateDefWizard
          catalog={catalog}
          session={editorSession}
          onClose={() => setWizardOpen(false)}
          onCreated={(result) => {
            setWizardOpen(false);
            if (result.insertedNodeId != null) {
              editorSession.selectDef(result.insertedNodeId);
            }
          }}
        />
      )}

      {/* Save as Template dialog */}
      {saveTemplateOpen && (
        <SaveDefTemplateDialog
          defaultName={defaultTemplateName}
          onSave={editorSession.saveSelectedDefAsTemplate}
          onClose={() => setSaveTemplateOpen(false)}
          onSaved={() => setSaveTemplateOpen(false)}
        />
      )}

      {/* Patch preview dialog */}
      {previewOpen && previewTarget && (
        <PatchPreviewDialog
          projectId={projectId}
          target={previewTarget}
          onClose={() => setPreviewOpen(false)}
        />
      )}
    </div>
  );
}
