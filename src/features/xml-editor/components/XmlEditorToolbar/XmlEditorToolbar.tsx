import { useTranslation } from "react-i18next";
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
  const { t } = useTranslation("editor");
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
    <div className={styles.toolbar} role="toolbar" aria-label={t("toolbar.ariaLabel")}>
      {/* Mode segmented control */}
      <div className={styles.modeSwitch} role="group" aria-label={t("toolbar.modeGroupAriaLabel")}>
        <ModeButton
          label={t("toolbar.formMode")}
          active={mode === "form"}
          disabled={!canSwitchToForm}
          onClick={() => switchMode("form")}
        />
        <ModeButton
          label={t("toolbar.xmlMode")}
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
          title={t("toolbar.newDef")}
          aria-label={t("toolbar.newDef")}
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
          title={t("toolbar.saveAsTemplate")}
          aria-label={t("toolbar.saveAsTemplate")}
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
          title={t("toolbar.previewPatches")}
          aria-label={t("toolbar.previewPatches")}
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
        title={t("toolbar.undo")}
        aria-label={t("toolbar.undo")}
      >
        <Undo2 size={14} />
      </button>
      <button
        className={styles.iconBtn}
        onClick={redo}
        disabled={readOnly || !canRedo}
        title={t("toolbar.redo")}
        aria-label={t("toolbar.redo")}
      >
        <Redo2 size={14} />
      </button>

      <div className={styles.spacer} />

      {/* Dirty indicator */}
      {dirty && (
        <span
          className={styles.dirtyDot}
          title={t("toolbar.unsavedChanges")}
          aria-label={t("toolbar.unsavedChanges")}
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
          title={t("toolbar.previewSave")}
        >
          <Eye size={13} />
          {t("toolbar.previewSave")}
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
