import { Undo2, Redo2, Eye, FilePlus, BookmarkPlus, History } from "lucide-react";
import type { UseXmlEditorSessionReturn } from "../../hooks/useXmlEditorSession";
import { measureAsync, generateTraceId } from "../../../../instrumentation";
import styles from "./XmlEditorToolbar.module.css";

interface Props {
  session: UseXmlEditorSessionReturn;
  onCreateDef?: () => void;
  canCreateDef?: boolean;
  onSaveAsTemplate?: () => void;
  canSaveAsTemplate?: boolean;
  onPreviewPatches?: () => void;
  canPreviewPatches?: boolean;
}

export function XmlEditorToolbar({
  session,
  onCreateDef,
  canCreateDef = false,
  onSaveAsTemplate,
  canSaveAsTemplate = false,
  onPreviewPatches,
  canPreviewPatches = false,
}: Props) {
  const {
    mode,
    relativePath,
    dirty,
    canUndo,
    canRedo,
    readOnly,
    lastValidSnapshot,
    currentRawXml,
    isBufferValid,
    saveBusy,
    switchMode,
    undo,
    redo,
    requestSavePreview,
  } = session;

  const canSwitchToForm = lastValidSnapshot?.parsed != null;

  return (
    <div className={styles.toolbar} role="toolbar" aria-label="Editor toolbar">
      {/* Mode segmented control */}
      <div className={styles.modeSwitch} role="group" aria-label="Editor mode">
        <ModeButton
          label="Form"
          active={mode === "form"}
          disabled={!canSwitchToForm}
          onClick={() => switchMode("form")}
        />
        <ModeButton
          label="XML"
          active={mode === "raw"}
          disabled={false}
          onClick={() => switchMode("raw")}
        />
      </div>

      <div className={styles.sep} />

      {/* New Def - hidden for read-only files */}
      {!readOnly && onCreateDef && (
        <button
          className={styles.iconBtn}
          onClick={onCreateDef}
          disabled={!canCreateDef}
          title="New Def"
          aria-label="New Def"
        >
          <FilePlus size={14} />
        </button>
      )}

      {/* Save as Template - hidden for read-only files */}
      {!readOnly && onSaveAsTemplate && (
        <button
          className={styles.iconBtn}
          onClick={onSaveAsTemplate}
          disabled={!canSaveAsTemplate}
          title="Save as Template"
          aria-label="Save as Template"
        >
          <BookmarkPlus size={14} />
        </button>
      )}

      {/* Preview Patches - a read-only view, available even for read-only files */}
      {onPreviewPatches && (
        <button
          className={styles.iconBtn}
          onClick={onPreviewPatches}
          disabled={!canPreviewPatches}
          title="Preview Patches"
          aria-label="Preview Patches"
        >
          <History size={14} />
        </button>
      )}

      <div className={styles.sep} />

      {/* Undo / Redo */}
      <button
        className={styles.iconBtn}
        onClick={undo}
        disabled={readOnly || !canUndo}
        title="Undo"
        aria-label="Undo"
      >
        <Undo2 size={14} />
      </button>
      <button
        className={styles.iconBtn}
        onClick={redo}
        disabled={readOnly || !canRedo}
        title="Redo"
        aria-label="Redo"
      >
        <Redo2 size={14} />
      </button>

      <div className={styles.spacer} />

      {/* Dirty indicator */}
      {dirty && (
        <span
          className={styles.dirtyDot}
          title="Unsaved changes"
          aria-label="Unsaved changes"
        />
      )}

      {/* Preview Save - hidden for read-only files */}
      {!readOnly && (
        <button
          className={styles.previewBtn}
          onClick={() => {
            const startedAt = performance.now();
            const traceId = generateTraceId();
            void measureAsync(
              "xmlEditor.previewSave.clickToRequestComplete",
              () => requestSavePreview(traceId, "toolbar", startedAt),
              { traceId, relativePath, mode, source: "toolbar" },
            );
          }}
          disabled={saveBusy || !isBufferValid || !currentRawXml}
          title="Preview Save"
        >
          <Eye size={13} />
          Preview Save
        </button>
      )}
    </div>
  );
}

interface ModeButtonProps {
  label: string;
  active: boolean;
  disabled: boolean;
  onClick: () => void;
}

function ModeButton({ label, active, disabled, onClick }: ModeButtonProps) {
  return (
    <button
      className={`${styles.modeBtn} ${active ? styles.modeBtnActive : ""}`}
      onClick={onClick}
      disabled={disabled}
      aria-pressed={active}
    >
      {label}
    </button>
  );
}
