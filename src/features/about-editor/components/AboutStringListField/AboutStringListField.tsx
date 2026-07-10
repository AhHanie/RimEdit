import { useEffect, useState } from "react";
import { Plus, X } from "lucide-react";
import styles from "./AboutStringListField.module.css";

interface Props {
  label: string;
  items: string[];
  readOnly: boolean;
  placeholder?: string;
  onCommit: (items: string[]) => void;
}

/** Tag-list editor reused for every string-list About field (authors, supportedVersions,
 * loadBefore/loadAfter/forceLoadBefore/forceLoadAfter, incompatibleWith, alternativePackageIds). */
export function AboutStringListField({ label, items, readOnly, placeholder, onCommit }: Props) {
  const [draft, setDraft] = useState(items);
  const [newValue, setNewValue] = useState("");

  useEffect(() => {
    setDraft(items);
  }, [items]);

  function commit(next: string[]) {
    setDraft(next);
    onCommit(next);
  }

  function addValue() {
    const trimmed = newValue.trim();
    if (!trimmed) return;
    commit([...draft, trimmed]);
    setNewValue("");
  }

  return (
    <div className={styles.field}>
      <span className={styles.label}>{label}</span>
      {draft.length > 0 && (
        <ul className={styles.tags}>
          {draft.map((item, i) => (
            <li key={`${item}-${i}`} className={styles.tag}>
              <span>{item}</span>
              {!readOnly && (
                <button
                  type="button"
                  className={styles.remove}
                  onClick={() => commit(draft.filter((_, idx) => idx !== i))}
                  aria-label={`Remove ${item}`}
                >
                  <X size={10} />
                </button>
              )}
            </li>
          ))}
        </ul>
      )}
      {!readOnly && (
        <div className={styles.addRow}>
          <input
            className={styles.input}
            type="text"
            value={newValue}
            placeholder={placeholder}
            onChange={(e) => setNewValue(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                addValue();
              }
            }}
          />
          <button
            type="button"
            className={styles.addBtn}
            disabled={!newValue.trim()}
            onClick={addValue}
            aria-label={`Add to ${label}`}
          >
            <Plus size={12} />
          </button>
        </div>
      )}
    </div>
  );
}
