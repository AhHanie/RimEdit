import { useEffect, useState } from "react";
import { X } from "lucide-react";
import { readIndexedDefXml } from "../../../def-index/api/defIndex";
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
  const [rawXml, setRawXml] = useState<string | null>(null);
  const [defLine, setDefLine] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    readIndexedDefXml(projectId, locationId, relativePath, defType, defName)
      .then((preview) => {
        setRawXml(preview.rawXml);
        setDefLine(preview.defLine ?? null);
      })
      .catch((e: unknown) => setError(String(e)));
  }, [projectId, locationId, relativePath, defType, defName]);

  function handleOverlayClick(e: React.MouseEvent) {
    if (e.target === e.currentTarget) onClose();
  }

  const subtitle = defLine
    ? `${relativePath} · full file shown, Def at line ${defLine}`
    : `${relativePath} · full file shown`;

  return (
    <div className={styles.dialogOverlay} onClick={handleOverlayClick}>
      <div className={styles.dialog} role="dialog" aria-modal="true">
        <div className={styles.dialogHeader}>
          <div>
            <div className={styles.dialogTitle}>{defType}: {defName}</div>
            <div className={styles.dialogSubtitle}>{subtitle}</div>
          </div>
          <button className={styles.dialogClose} onClick={onClose} aria-label="Close" type="button">
            <X size={14} />
          </button>
        </div>
        <div className={styles.dialogBody}>
          {!rawXml && !error && <p className={styles.dialogLoading}>Loading…</p>}
          {error && <p className={styles.dialogError}>{error}</p>}
          {rawXml && <pre className={styles.xmlPre}>{rawXml}</pre>}
        </div>
      </div>
    </div>
  );
}
