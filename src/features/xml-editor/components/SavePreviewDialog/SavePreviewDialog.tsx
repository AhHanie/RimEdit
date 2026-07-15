import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { X } from "lucide-react";
import type { UseXmlEditorSessionReturn } from "../../hooks/useXmlEditorSession";
import { markTiming } from "../../../../instrumentation";
import styles from "./SavePreviewDialog.module.css";

interface Props {
  session: UseXmlEditorSessionReturn;
}

export function SavePreviewDialog({ session }: Props) {
  // Two separate single-namespace hooks, not `useTranslation(["editor", "common"])` with
  // `"common:key"`-prefixed lookups -- see `AboutDependencySection`'s `DependencyRow` doc comment.
  const { t } = useTranslation("editor");
  const { t: tCommon } = useTranslation("common");
  const {
    savePreview,
    saveError,
    saveBusy,
    relativePath,
    confirmSave,
    clearSavePreview,
    loadFullSavePreview,
    savePreviewTraceId,
    savePreviewStartedAt,
  } = session;

  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (savePreviewStartedAt == null) return;
    markTiming(
      "xmlEditor.previewSave.dialogRendered",
      performance.now() - savePreviewStartedAt,
      {
        relativePath,
        diffLines: savePreview?.diff.length ?? 0,
        changed: savePreview?.changed ?? false,
        ...(savePreviewTraceId != null ? { traceId: savePreviewTraceId } : {}),
      },
    );
  }, []); // fires once on dialog mount

  if (!savePreview) return null;

  // The default preview is hunked (changed lines ± context); gap markers indicate elided
  // unchanged regions. Offer a full-file view on demand (re-fetches the uncollapsed diff).
  const hasCollapsedRegions = savePreview.diff.some(
    (line) => line.kind === "gap",
  );

  return (
    <div
      className={styles.overlay}
      role="dialog"
      aria-modal="true"
      aria-label={t("savePreviewDialog.dialogAriaLabel")}
    >
      <div className={styles.panel}>
        <div className={styles.header}>
          <span className={styles.title}>
            {t("savePreviewDialog.title", { relativePath })}
          </span>
          <button
            className={styles.closeBtn}
            onClick={clearSavePreview}
            aria-label={t("savePreviewDialog.closePreview")}
          >
            <X size={14} />
          </button>
        </div>

        {!savePreview.changed && (
          <p className={styles.unchanged}>{t("savePreviewDialog.noChanges")}</p>
        )}

        {hasCollapsedRegions && (
          <div className={styles.diffToolbar}>
            <button
              className={styles.showFullBtn}
              onClick={() => void loadFullSavePreview()}
              disabled={saveBusy}
              type="button"
            >
              {t("savePreviewDialog.showFullFile")}
            </button>
          </div>
        )}

        <div className={styles.diffArea}>
          {savePreview.diff.map((line, i) => {
            // Collapsed run of unchanged lines - a single muted separator instead of ~4N DOM nodes.
            if (line.kind === "gap") {
              return (
                <div key={i} className={`${styles.diffLine} ${styles.gap}`}>
                  ⋯ {t("savePreviewDialog.unchangedLinesCount", { count: line.count ?? 0 })} ⋯
                </div>
              );
            }
            return (
              <div
                key={i}
                className={`${styles.diffLine} ${
                  line.kind === "added"
                    ? styles.added
                    : line.kind === "removed"
                      ? styles.removed
                      : styles.unchanged
                }`}
              >
                <span className={styles.lineNum}>
                  {line.kind === "removed" ? line.oldLine : line.newLine}
                </span>
                <span className={styles.linePrefix}>
                  {line.kind === "added"
                    ? "+"
                    : line.kind === "removed"
                      ? "-"
                      : " "}
                </span>
                <span className={styles.lineText}>{line.text}</span>
              </div>
            );
          })}
        </div>

        {saveError && <p className={styles.error}>{saveError}</p>}

        <div className={styles.actions}>
          <button className={styles.cancelBtn} onClick={clearSavePreview}>
            {tCommon("actions.cancel")}
          </button>
          <button
            className={styles.saveBtn}
            onClick={() => void confirmSave(undefined, "dialog")}
            disabled={saveBusy || !savePreview.changed}
          >
            {saveBusy ? t("savePreviewDialog.saving") : tCommon("actions.save")}
          </button>
        </div>
      </div>
    </div>
  );
}
