import { useState, type ReactNode } from "react";
import { RotateCcw, X } from "lucide-react";
import type { FieldSchema } from "../../../schema-catalog";
import styles from "./ObjectFieldControl.module.css";

interface ObjectFieldControlProps {
  fieldName: string;
  fieldSchema: FieldSchema;
  dirty: boolean;
  onReset: (() => void) | null;
  onClear: (() => void) | null;
  error?: string | null;
  children: ReactNode;
}

export function ObjectFieldControl({
  fieldName,
  fieldSchema,
  dirty,
  onReset,
  onClear,
  error,
  children,
}: ObjectFieldControlProps) {
  const label = fieldSchema.label ?? fieldName;
  const [touched, setTouched] = useState(false);

  return (
    <div
      className={`${styles.field}${dirty ? ` ${styles.dirty}` : ""}`}
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) {
          setTouched(true);
        }
      }}
    >
      <div className={styles.labelRow}>
        <label className={styles.label}>
          {label}
          {fieldSchema.required && <span className={styles.required}>*</span>}
        </label>
        <div className={styles.actions}>
          {dirty && onReset && (
            <button
              type="button"
              className={styles.resetBtn}
              onClick={onReset}
              title="Reset field"
              aria-label={`Reset ${label}`}
            >
              <RotateCcw size={12} />
            </button>
          )}
          {onClear && (
            <button
              type="button"
              className={styles.clearBtn}
              onClick={onClear}
              title={`Clear ${label}`}
              aria-label={`Clear ${label}`}
            >
              <X size={12} />
            </button>
          )}
        </div>
      </div>
      {fieldSchema.description && <p className={styles.description}>{fieldSchema.description}</p>}
      {children}
      {fieldSchema.examples.length > 0 && (
        <p className={styles.hint}>e.g. {fieldSchema.examples.slice(0, 2).join(", ")}</p>
      )}
      {touched && error && <p className={styles.error}>{error}</p>}
    </div>
  );
}
