import { useEffect, useState } from "react";
import type { ValidationDiagnostic } from "../../../xml-editor/types/xmlDocument";
import { fieldSeverity } from "../../lib/aboutValidationText";
import styles from "./AboutTextField.module.css";

interface Props {
  label: string;
  value: string;
  readOnly: boolean;
  multiline?: boolean;
  placeholder?: string;
  diagnostics?: ValidationDiagnostic[];
  onCommit: (value: string) => void;
}

export function AboutTextField({
  label,
  value,
  readOnly,
  multiline,
  placeholder,
  diagnostics = [],
  onCommit,
}: Props) {
  const [draft, setDraft] = useState(value);

  useEffect(() => {
    setDraft(value);
  }, [value]);

  const severity = fieldSeverity(diagnostics);

  function commitIfChanged() {
    if (!readOnly && draft !== value) onCommit(draft);
  }

  return (
    <label className={styles.field}>
      <span className={styles.labelRow}>
        <span className={styles.label}>{label}</span>
        {severity && (
          <span
            className={`${styles.badge} ${severity === "Error" ? styles.badgeError : styles.badgeWarning}`}
            title={diagnostics.map((d) => d.message).join("\n")}
          >
            {severity === "Error" ? "!" : "?"}
          </span>
        )}
      </span>
      {multiline ? (
        <textarea
          className={styles.input}
          value={draft}
          placeholder={placeholder}
          readOnly={readOnly}
          rows={3}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={commitIfChanged}
        />
      ) : (
        <input
          className={styles.input}
          type="text"
          value={draft}
          placeholder={placeholder}
          readOnly={readOnly}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={commitIfChanged}
        />
      )}
    </label>
  );
}
