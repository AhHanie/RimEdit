import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { CheckCircle2, AlertCircle, Loader2 } from "lucide-react";
import { IndexErrorsDialog, type IndexingStatus } from "../../../features/def-index";
import { formatFileSize, formatNumber } from "../../../i18n/format";
import styles from "./StatusBar.module.css";

function runningStatusLabel(
  status: IndexingStatus,
  t: TFunction<"shell">,
  locale: string,
): string {
  if (status.phase === "pending") return t("statusBar.indexPending");
  if (status.currentStage === "discovering") return t("statusBar.indexDiscovering");
  if (status.currentStage === "indexing" && status.totalFiles !== undefined) {
    return t("statusBar.indexingProgress", {
      processed: formatNumber(status.processedFiles ?? 0, { locale }),
      total: formatNumber(status.totalFiles, { locale }),
    });
  }
  return t("statusBar.indexing");
}

interface StatusBarProps {
  hasActiveProject: boolean;
  loadingScan: boolean;
  scanError: string | null;
  fileCount: number;
  activeFilePath: string | null;
  activeFileSizeBytes: number | null;
  indexingStatus?: IndexingStatus | null;
  projectId?: string;
}

export function StatusBar({
  hasActiveProject,
  loadingScan,
  scanError,
  fileCount,
  activeFilePath,
  activeFileSizeBytes,
  indexingStatus,
  projectId,
}: StatusBarProps) {
  const { t, i18n } = useTranslation(["shell", "common"]);
  const [errorsDialogOpen, setErrorsDialogOpen] = useState(false);
  return (
    <>
      <div className={styles.root} role="status" aria-live="polite">
        <div className={styles.segment}>
          {loadingScan ? (
            <>
              <Loader2 size={11} className={`${styles.icon} spin`} />
              <span>{t("shell:statusBar.scanning")}</span>
            </>
          ) : scanError ? (
            <>
              <AlertCircle size={11} className={styles.icon} />
              <span>{t("shell:statusBar.error")}</span>
            </>
          ) : hasActiveProject ? (
            <>
              <CheckCircle2 size={11} className={styles.icon} />
              <span>{t("shell:statusBar.ready")}</span>
            </>
          ) : (
            <span>{t("shell:statusBar.noProject")}</span>
          )}
        </div>

        {indexingStatus && indexingStatus.phase !== "idle" && (
          <>
            <span className={styles.divider}>|</span>
            <div className={styles.segment}>
              {indexingStatus.phase === "pending" || indexingStatus.phase === "running" ? (
                <>
                  <Loader2 size={11} className={`${styles.icon} spin`} />
                  <span>{runningStatusLabel(indexingStatus, t, i18n.language)}</span>
                </>
              ) : indexingStatus.phase === "complete" && indexingStatus.errors > 0 ? (
                <button
                  type="button"
                  className={styles.indexErrorsBtn}
                  onClick={() => setErrorsDialogOpen(true)}
                  aria-label={t("shell:statusBar.indexErrors", { count: indexingStatus.errors })}
                >
                  <AlertCircle size={11} className={styles.icon} />
                  <span>{t("shell:statusBar.indexErrors", { count: indexingStatus.errors })}</span>
                </button>
              ) : indexingStatus.phase === "complete" ? (
                <>
                  <CheckCircle2 size={11} className={styles.icon} />
                  <span>
                    {t("shell:statusBar.indexed", { count: indexingStatus.indexedDefs })}
                  </span>
                </>
              ) : indexingStatus.phase === "failed" ? (
                <>
                  <AlertCircle size={11} className={styles.icon} />
                  <span>{t("shell:statusBar.indexFailed")}</span>
                </>
              ) : null}
            </div>
          </>
        )}

        {fileCount > 0 && (
          <>
            <span className={styles.divider}>|</span>
            <div className={styles.segment}>
              <span>{t("shell:statusBar.filesCount", { count: fileCount })}</span>
            </div>
          </>
        )}

        {activeFilePath && (
          <>
            <span className={styles.divider}>|</span>
            <div className={`${styles.segment} ${styles.segmentPath}`}>
              <span title={activeFilePath}>{activeFilePath}</span>
            </div>
          </>
        )}

        {activeFileSizeBytes !== null && (
          <>
            <span className={styles.divider}>|</span>
            <div className={styles.segment}>
              <span>{formatFileSize(activeFileSizeBytes, { locale: i18n.language })}</span>
            </div>
          </>
        )}

        <div className={styles.right}>
          <div className={styles.segment}>
            <span>{t("shell:statusBar.xmlLabel")}</span>
            <span className={styles.divider}>•</span>
            <span>{t("shell:statusBar.readOnlyLabel")}</span>
          </div>
        </div>
      </div>

      {errorsDialogOpen && projectId && (
        <IndexErrorsDialog projectId={projectId} onClose={() => setErrorsDialogOpen(false)} />
      )}
    </>
  );
}
