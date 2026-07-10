import { CheckCircle2, AlertCircle, Loader2 } from "lucide-react";
import type { ThemeMode } from "../../../types/ui";
import type { IndexingStatus } from "../../../features/def-index";
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

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  return `${(bytes / 1024).toFixed(1)} KB`;
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
  return (
    <div className={styles.root} role="status" aria-live="polite">
      <div className={styles.segment}>
        {loadingScan ? (
          <>
            <Loader2 size={11} className={`${styles.icon} spin`} />
            <span>Scanning…</span>
          </>
        ) : scanError ? (
          <>
            <AlertCircle size={11} className={styles.icon} />
            <span>Error</span>
          </>
        ) : hasActiveProject ? (
          <>
            <CheckCircle2 size={11} className={styles.icon} />
            <span>Ready</span>
          </>
        ) : (
          <span>No project</span>
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
                  {indexingStatus.phase === "pending" ? "Index pending" : "Indexing…"}
                </span>
              </>
            ) : indexingStatus.phase === "complete" && indexingStatus.errors > 0 ? (
              <>
                <AlertCircle size={11} className={styles.icon} />
                <span>
                  {indexingStatus.errors}{" "}
                  {indexingStatus.errors === 1 ? "index error" : "index errors"}
                </span>
              </>
            ) : indexingStatus.phase === "complete" ? (
              <>
                <CheckCircle2 size={11} className={styles.icon} />
                <span>Indexed {indexingStatus.indexedDefs} defs</span>
              </>
            ) : indexingStatus.phase === "failed" ? (
              <>
                <AlertCircle size={11} className={styles.icon} />
                <span>Index failed</span>
              </>
            ) : null}
          </div>
        </>
      )}

      {fileCount > 0 && (
        <>
          <span className={styles.divider}>|</span>
          <div className={styles.segment}>
            <span>
              {fileCount} {fileCount === 1 ? "file" : "files"}
            </span>
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
            <span>{formatSize(activeFileSizeBytes)}</span>
          </div>
        </>
      )}

      <div className={styles.right}>
        <div className={styles.segment}>
          <span>XML</span>
          <span className={styles.divider}>•</span>
          <span>Read-only</span>
        </div>
      </div>
    </div>
  );
}
