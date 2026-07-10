import { useRef, useState } from "react";
import { X } from "lucide-react";
import { formatError } from "../../../../lib/formatError";
import type { UserDefTemplate } from "../../types/defTemplates";
import styles from "./SaveDefTemplateDialog.module.css";

interface Props {
  defaultName: string;
  onSave: (name: string) => Promise<UserDefTemplate>;
  onClose: () => void;
  onSaved: (template: UserDefTemplate) => void;
}

export function SaveDefTemplateDialog({
  defaultName,
  onSave,
  onClose,
  onSaved,
}: Props) {
  const [name, setName] = useState(defaultName);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const trimmedName = name.trim();

  async function handleSave() {
    if (!trimmedName || busy) return;
    setBusy(true);
    setError(null);
    try {
      const template = await onSave(trimmedName);
      onSaved(template);
    } catch (e: unknown) {
      setError(formatError(e));
      setBusy(false);
    }
  }

  return (
    <div
      className={styles.overlay}
      role="dialog"
      aria-modal="true"
      aria-label="Save as Template"
    >
      <div className={styles.panel}>
        <div className={styles.header}>
          <span className={styles.title}>Save as Template</span>
          <button className={styles.closeBtn} onClick={onClose} aria-label="Close">
            <X size={14} />
          </button>
        </div>

        <div className={styles.body}>
          <div className={styles.fieldRow}>
            <label className={styles.fieldLabel} htmlFor="save-def-template-name">
              Template name
            </label>
            <input
              ref={inputRef}
              id="save-def-template-name"
              className={styles.fieldInput}
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void handleSave();
              }}
              autoFocus
              autoComplete="off"
            />
          </div>
        </div>

        <div className={styles.footer}>
          {error ? (
            <span className={styles.errorBanner} title={error}>
              {error}
            </span>
          ) : (
            <span className={styles.spacer} />
          )}
          <button className={styles.cancelBtn} onClick={onClose} disabled={busy}>
            Cancel
          </button>
          <button
            className={styles.saveBtn}
            onClick={() => void handleSave()}
            disabled={busy || !trimmedName}
          >
            {busy ? "Saving…" : "Save"}
          </button>
        </div>
      </div>
    </div>
  );
}
