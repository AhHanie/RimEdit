import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { getVersion } from "@tauri-apps/api/app";
import { X } from "lucide-react";
import { useDialogKeyboard } from "../../../lib/useDialogKeyboard";
import appIcon from "../../../assets/app-icon.png";
import styles from "./AboutDialog.module.css";

interface AboutDialogProps {
  onClose: () => void;
}

type VersionState =
  | { status: "loading" }
  | { status: "resolved"; version: string }
  | { status: "unavailable" };

/**
 * Help > About dialog: shows the application name and the packaged Tauri app version, read at
 * runtime via `getVersion()` so it always reflects `src-tauri/tauri.conf.json` instead of a
 * hardcoded frontend copy (Plan.md section 5). Running under the Vite-only dev server (no Tauri
 * backend) has no IPC to answer the call, so a rejection falls back to a localized "unavailable"
 * state rather than surfacing an unhandled promise rejection or blocking the dialog from closing.
 */
export function AboutDialog({ onClose }: AboutDialogProps) {
  const { t } = useTranslation(["shell", "common"]);
  const [versionState, setVersionState] = useState<VersionState>({ status: "loading" });
  const containerRef = useRef<HTMLDivElement>(null);
  useDialogKeyboard(containerRef, onClose);

  useEffect(() => {
    let cancelled = false;
    getVersion()
      .then((version) => {
        if (!cancelled) setVersionState({ status: "resolved", version });
      })
      .catch(() => {
        if (!cancelled) setVersionState({ status: "unavailable" });
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div
      className={styles.overlay}
      role="dialog"
      aria-modal="true"
      aria-label={t("shell:aboutDialog.dialogAriaLabel")}
      onClick={onClose}
    >
      <div className={styles.panel} ref={containerRef} onClick={(e) => e.stopPropagation()}>
        <div className={styles.header}>
          <span className={styles.title}>{t("shell:aboutDialog.title")}</span>
          <button
            className={styles.closeBtn}
            onClick={onClose}
            aria-label={t("common:actions.close")}
          >
            <X size={14} />
          </button>
        </div>

        <div className={styles.body}>
          <img src={appIcon} alt="" className={styles.appIcon} aria-hidden="true" />
          <span className={styles.appName}>{t("common:app.name")}</span>
          <div className={styles.versionRow}>
            <span className={styles.versionLabel}>{t("shell:aboutDialog.versionLabel")}</span>
            <span className={styles.versionValue}>
              {versionState.status === "loading" && t("shell:aboutDialog.versionLoading")}
              {versionState.status === "resolved" && versionState.version}
              {versionState.status === "unavailable" && t("shell:aboutDialog.versionUnavailable")}
            </span>
          </div>
        </div>

        <div className={styles.footer}>
          <button className={styles.closeFooterBtn} onClick={onClose}>
            {t("common:actions.close")}
          </button>
        </div>
      </div>
    </div>
  );
}
