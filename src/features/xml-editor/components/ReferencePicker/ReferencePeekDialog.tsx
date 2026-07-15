import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { X } from "lucide-react";
import { readIndexedDefXml } from "../../../def-index/api/defIndex";
import { formatError } from "../../../../lib/formatError";
import styles from "./ReferencePicker.module.css";

interface Props {
  projectId: string;
  locationId: string;
  relativePath: string;
  defName: string;
  defType: string;
  onClose: () => void;
}

export function ReferencePeekDialog({ projectId, locationId, relativePath, defName, defType, onClose }: Props) {
  // Two separate single-namespace hooks, not `useTranslation(["editor", "common"])` with
  // `"common:key"`-prefixed lookups -- see `AboutDependencySection`'s `DependencyRow` doc comment.
  const { t } = useTranslation("editor");
  const { t: tCommon } = useTranslation("common");
  const [rawXml, setRawXml] = useState<string | null>(null);
  const [defLine, setDefLine] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    readIndexedDefXml(projectId, locationId, relativePath, defType, defName)
      .then((preview) => {
        setRawXml(preview.rawXml);
        setDefLine(preview.defLine ?? null);
      })
      .catch((e: unknown) => setError(formatError(e)));
  }, [projectId, locationId, relativePath, defType, defName]);

  function handleOverlayClick(e: React.MouseEvent) {
    if (e.target === e.currentTarget) onClose();
  }

  const subtitle = defLine
    ? t("referencePicker.peekSubtitleWithLine", { relativePath, line: defLine })
    : t("referencePicker.peekSubtitle", { relativePath });

  return (
    <div className={styles.dialogOverlay} onClick={handleOverlayClick}>
      <div className={styles.dialog} role="dialog" aria-modal="true">
        <div className={styles.dialogHeader}>
          <div>
            <div className={styles.dialogTitle}>
              {t("referencePicker.peekTitle", { defType, defName })}
            </div>
            <div className={styles.dialogSubtitle}>{subtitle}</div>
          </div>
          <button
            className={styles.dialogClose}
            onClick={onClose}
            aria-label={tCommon("actions.close")}
            type="button"
          >
            <X size={14} />
          </button>
        </div>
        <div className={styles.dialogBody}>
          {!rawXml && !error && <p className={styles.dialogLoading}>{t("referencePicker.loading")}</p>}
          {error && <p className={styles.dialogError}>{error}</p>}
          {rawXml && <pre className={styles.xmlPre}>{rawXml}</pre>}
        </div>
      </div>
    </div>
  );
}
