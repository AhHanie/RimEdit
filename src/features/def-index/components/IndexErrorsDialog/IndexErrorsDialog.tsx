import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { i18n as I18nInstance, TFunction } from "i18next";
import { AlertCircle, Copy, Loader2, X } from "lucide-react";
import { useDialogKeyboard } from "../../../../lib/useDialogKeyboard";
import { renderDiagnostic } from "../../../../i18n/diagnostics";
import { getDefIndexErrors } from "../../api/defIndex";
import type { DefIndexError } from "../../types";
import styles from "./IndexErrorsDialog.module.css";

interface IndexErrorsDialogProps {
  projectId: string;
  onClose: () => void;
}

type LoadState =
  | { status: "loading" }
  | { status: "loaded"; errors: DefIndexError[]; total: number; truncated: boolean }
  | { status: "error" };

function describeLocation(t: TFunction<"shell">, error: DefIndexError): string {
  return error.relativePath ?? t("indexErrorsDialog.locationScan");
}

function describePosition(error: DefIndexError): string | null {
  if (error.line === undefined) return null;
  return error.column !== undefined ? `${error.line}:${error.column}` : `${error.line}`;
}

function describeMessage(error: DefIndexError, i18nInstance: I18nInstance): string {
  return renderDiagnostic(
    { code: error.code, args: error.args, message: error.message },
    i18nInstance,
  );
}

/**
 * Data-only errors panel: fetches on open (not on status-bar polling) via
 * `getDefIndexErrors`, and never exposes the location root or XML content -- only the
 * already-redacted fields persisted on `DefIndexError` (Plan.md section 2).
 */
export function IndexErrorsDialog({ projectId, onClose }: IndexErrorsDialogProps) {
  const { t, i18n } = useTranslation(["shell", "common"]);
  const [state, setState] = useState<LoadState>({ status: "loading" });
  const [copied, setCopied] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  useDialogKeyboard(containerRef, onClose);

  useEffect(() => {
    let cancelled = false;
    setState({ status: "loading" });
    getDefIndexErrors(projectId)
      .then((response) => {
        if (cancelled) return;
        setState({
          status: "loaded",
          errors: response.errors,
          total: response.total,
          truncated: response.truncated,
        });
      })
      .catch(() => {
        if (!cancelled) setState({ status: "error" });
      });
    return () => {
      cancelled = true;
    };
  }, [projectId]);

  async function handleCopy() {
    if (state.status !== "loaded") return;
    const lines = state.errors.map((error) => {
      const position = describePosition(error);
      const path = describeLocation(t, error);
      return `[${error.locationName}] ${path}${position ? `:${position}` : ""} (${error.code}) ${describeMessage(error, i18n)}`;
    });
    try {
      await navigator.clipboard.writeText(lines.join("\n"));
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Clipboard unavailable (e.g. permission denied) -- nothing else to fall back to here.
    }
  }

  return (
    <div
      className={styles.overlay}
      role="dialog"
      aria-modal="true"
      aria-label={t("shell:indexErrorsDialog.dialogAriaLabel")}
      onClick={onClose}
    >
      <div className={styles.panel} ref={containerRef} onClick={(e) => e.stopPropagation()}>
        <div className={styles.header}>
          <span className={styles.title}>{t("shell:indexErrorsDialog.title")}</span>
          <button
            className={styles.closeBtn}
            onClick={onClose}
            aria-label={t("common:actions.close")}
          >
            <X size={14} />
          </button>
        </div>

        <div className={styles.body}>
          {state.status === "loading" && (
            <div className="state-loading">
              <Loader2 size={14} className="spin" />
              <span>{t("shell:indexErrorsDialog.loading")}</span>
            </div>
          )}

          {state.status === "error" && (
            <div className={styles.errorState}>
              <AlertCircle size={14} />
              <span>{t("shell:indexErrorsDialog.loadFailed")}</span>
            </div>
          )}

          {state.status === "loaded" && state.errors.length === 0 && (
            <p className={styles.empty}>{t("shell:indexErrorsDialog.empty")}</p>
          )}

          {state.status === "loaded" && state.errors.length > 0 && (
            <>
              {state.truncated && (
                <p className={styles.truncatedNotice}>
                  {t("shell:indexErrorsDialog.truncatedNotice", {
                    shown: state.errors.length,
                    total: state.total,
                  })}
                </p>
              )}
              <ul
                className={styles.list}
                aria-label={t("shell:indexErrorsDialog.listAriaLabel")}
              >
                {state.errors.map((error, i) => {
                  const position = describePosition(error);
                  return (
                    <li
                      key={`${error.locationId}:${error.relativePath ?? ""}:${error.code}:${i}`}
                      className={styles.item}
                    >
                      <div className={styles.itemHeader}>
                        <span className={styles.locationName}>{error.locationName}</span>
                        <span className={styles.code}>{error.code}</span>
                      </div>
                      <div className={styles.path}>
                        {describeLocation(t, error)}
                        {position && <span className={styles.position}>:{position}</span>}
                      </div>
                      <div className={styles.message}>{describeMessage(error, i18n)}</div>
                    </li>
                  );
                })}
              </ul>
            </>
          )}
        </div>

        <div className={styles.footer}>
          {state.status === "loaded" && state.errors.length > 0 && (
            <button className={styles.copyBtn} onClick={() => void handleCopy()}>
              <Copy size={13} />
              {copied
                ? t("shell:indexErrorsDialog.copied")
                : t("shell:indexErrorsDialog.copyDetails")}
            </button>
          )}
          <button className={styles.closeFooterBtn} onClick={onClose}>
            {t("common:actions.close")}
          </button>
        </div>
      </div>
    </div>
  );
}
