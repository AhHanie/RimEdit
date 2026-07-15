import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
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
  // Two separate single-namespace hooks, not `useTranslation(["editor", "common"])` with
  // `"common:key"`-prefixed lookups -- see `AboutDependencySection`'s `DependencyRow` doc comment.
  const { t } = useTranslation("editor");
  const { t: tCommon } = useTranslation("common");
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
      aria-label={t("saveDefTemplateDialog.dialogAriaLabel")}
    >
      <div className={styles.panel}>
        <div className={styles.header}>
          <span className={styles.title}>{t("saveDefTemplateDialog.title")}</span>
          <button className={styles.closeBtn} onClick={onClose} aria-label={tCommon("actions.close")}>
            <X size={14} />
          </button>
        </div>

        <div className={styles.body}>
          <div className={styles.fieldRow}>
            <label className={styles.fieldLabel} htmlFor="save-def-template-name">
              {t("saveDefTemplateDialog.templateNameLabel")}
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
            {tCommon("actions.cancel")}
          </button>
          <button
            className={styles.saveBtn}
            onClick={() => void handleSave()}
            disabled={busy || !trimmedName}
          >
            {busy ? t("saveDefTemplateDialog.saving") : tCommon("actions.save")}
          </button>
        </div>
      </div>
    </div>
  );
}
