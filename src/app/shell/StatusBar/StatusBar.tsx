import { useTranslation } from "react-i18next";
import { CheckCircle2, AlertCircle, Loader2 } from "lucide-react";
import type { ThemeMode } from "../../../types/ui";
import type { IndexingStatus } from "../../../features/def-index";
import { formatFileSize } from "../../../i18n/format";
import styles from "./StatusBar.module.css";

interface StatusBarProps {
  hasActiveProject: boolean;
  loadingScan: boolean;
  scanError: string | null;
  fileCount: number;
  activeFilePath: string | null;
  activeFileSizeBytes: number | null;
  themeMode: ThemeMode;
  indexingStatus?: IndexingStatus | null;
}

export function StatusBar({
  hasActiveProject,
  loadingScan,
  scanError,
  fileCount,
  activeFilePath,
  activeFileSizeBytes,
  indexingStatus,
}: StatusBarProps) {
  const { t, i18n } = useTranslation(["shell", "common"]);
  return (
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
                <span>
                  {indexingStatus.phase === "pending"
                    ? t("shell:statusBar.indexPending")
                    : t("shell:statusBar.indexing")}
                </span>
              </>
            ) : indexingStatus.phase === "complete" && indexingStatus.errors > 0 ? (
              <>
                <AlertCircle size={11} className={styles.icon} />
                <span>{t("shell:statusBar.indexErrors", { count: indexingStatus.errors })}</span>
              </>
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
  );
}
