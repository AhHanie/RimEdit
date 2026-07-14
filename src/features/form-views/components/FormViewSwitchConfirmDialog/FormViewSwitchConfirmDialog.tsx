import { useRef, useState } from "react";
import { X } from "lucide-react";
import { formatError } from "../../../../lib/formatError";
import { useDialogKeyboard } from "../../lib/useDialogKeyboard";
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
      aria-label="Unsaved view changes"
    >
      <div className={styles.panel} ref={containerRef}>
        <div className={styles.header}>
          <span className={styles.title}>Unsaved view changes</span>
          <button className={styles.closeBtn} onClick={onCancel} aria-label="Close">
            <X size={14} />
          </button>
        </div>

        <div className={styles.body}>
          {mode === "choose" ? (
            <p className={styles.message}>
              You have {hiddenCount} hidden field{hiddenCount === 1 ? "" : "s"} that
              aren&apos;t saved to any view yet. Switching views now will lose this change unless
              you save it first.
            </p>
          ) : (
            <div className={styles.fieldRow}>
              <label className={styles.fieldLabel} htmlFor="form-view-switch-save-name">
                Custom view name
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
                Cancel
              </button>
              <button
                className={styles.secondaryBtn}
                onClick={() => setMode("saveAs")}
              >
                Save as custom view
              </button>
              <button className={styles.dangerBtn} onClick={onDiscardAndSwitch}>
                Discard changes and switch
              </button>
            </>
          ) : (
            <>
              <button
                className={styles.cancelBtn}
                onClick={() => setMode("choose")}
                disabled={busy}
              >
                Back
              </button>
              <button
                className={styles.saveBtn}
                onClick={() => void handleSaveAsCustom()}
                disabled={busy || !trimmedName}
              >
                {busy ? "Saving…" : "Save and switch"}
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
