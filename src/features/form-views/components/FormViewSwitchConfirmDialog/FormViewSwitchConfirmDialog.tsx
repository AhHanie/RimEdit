import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { X } from "lucide-react";
import { formatError } from "../../../../lib/formatError";
import { useDialogKeyboard } from "../../../../lib/useDialogKeyboard";
import styles from "./FormViewSwitchConfirmDialog.module.css";

interface Props {
  /** Number of fields the dirty override currently hides -- shown so the user knows what's at
   * stake before discarding it (Plan.md section 8 step 6). */
  hiddenCount: number;
  onDiscardAndSwitch: () => void;
  onSaveAsCustom: (name: string) => Promise<void>;
  onCancel: () => void;
}

/**
 * The three-way decision required whenever a Form View selection changes (or the customize
 * dialog closes) while a Field Visibility Override is dirty (Plan.md section 8 step 6): discard
 * the override and switch, save it as a new custom view first, or cancel and keep editing. Never
 * auto-discards or auto-creates a custom view on its own.
 */
export function FormViewSwitchConfirmDialog({
  hiddenCount,
  onDiscardAndSwitch,
  onSaveAsCustom,
  onCancel,
}: Props) {
  // Two separate single-namespace hooks, not `useTranslation(["editor", "common"])` with
  // `"common:key"`-prefixed lookups -- see `AboutDependencySection`'s `DependencyRow` doc comment.
  const { t } = useTranslation("editor");
  const { t: tCommon } = useTranslation("common");
  const [mode, setMode] = useState<"choose" | "saveAs">("choose");
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  useDialogKeyboard(containerRef, onCancel);

  const trimmedName = name.trim();

  async function handleSaveAsCustom() {
    if (!trimmedName || busy) return;
    setBusy(true);
    setError(null);
    try {
      await onSaveAsCustom(trimmedName);
      // No `setBusy(false)` here on the success path: the normal case is that `onSaveAsCustom`
      // resolving also causes the parent to stop rendering this dialog (e.g.
      // `FormViewManagerDialog` clearing `closeConfirmPending`), so this component is about to
      // unmount anyway. But `onSaveAsCustom` can ALSO resolve successfully while deliberately
      // choosing not to dismiss this dialog -- a caller-side scope-staleness guard (the save
      // landed correctly, but the scope has since moved on, so the caller intentionally leaves
      // this dialog exactly as-is rather than dismissing something that no longer belongs to
      // the current scope). Without the unconditional `finally` below, THAT path would leave
      // `busy` stuck at `true` forever, permanently disabling Back/Save with no way out.
    } catch (e: unknown) {
      setError(formatError(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div
      className={styles.overlay}
      role="dialog"
      aria-modal="true"
      aria-label={t("formViews.switchConfirm.dialogAriaLabel")}
    >
      <div className={styles.panel} ref={containerRef}>
        <div className={styles.header}>
          <span className={styles.title}>{t("formViews.switchConfirm.title")}</span>
          <button className={styles.closeBtn} onClick={onCancel} aria-label={tCommon("actions.close")}>
            <X size={14} />
          </button>
        </div>

        <div className={styles.body}>
          {mode === "choose" ? (
            <p className={styles.message}>
              {t("formViews.switchConfirm.message", { count: hiddenCount })}
            </p>
          ) : (
            <div className={styles.fieldRow}>
              <label className={styles.fieldLabel} htmlFor="form-view-switch-save-name">
                {t("formViews.switchConfirm.customViewNameLabel")}
              </label>
              <input
                id="form-view-switch-save-name"
                className={styles.fieldInput}
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") void handleSaveAsCustom();
                }}
                autoFocus
                autoComplete="off"
              />
            </div>
          )}
        </div>

        <div className={styles.footer}>
          {error ? (
            <span className={styles.errorBanner} title={error}>
              {error}
            </span>
          ) : (
            <span className={styles.spacer} />
          )}
          {mode === "choose" ? (
            <>
              <button className={styles.cancelBtn} onClick={onCancel}>
                {tCommon("actions.cancel")}
              </button>
              <button
                className={styles.secondaryBtn}
                onClick={() => setMode("saveAs")}
              >
                {t("formViews.switchConfirm.saveAsCustomView")}
              </button>
              <button className={styles.dangerBtn} onClick={onDiscardAndSwitch}>
                {t("formViews.switchConfirm.discardAndSwitch")}
              </button>
            </>
          ) : (
            <>
              <button
                className={styles.cancelBtn}
                onClick={() => setMode("choose")}
                disabled={busy}
              >
                {t("formViews.switchConfirm.back")}
              </button>
              <button
                className={styles.saveBtn}
                onClick={() => void handleSaveAsCustom()}
                disabled={busy || !trimmedName}
              >
                {busy
                  ? t("formViews.switchConfirm.saving")
                  : t("formViews.switchConfirm.saveAndSwitch")}
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
