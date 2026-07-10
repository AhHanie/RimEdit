import { useMemo, useState } from "react";
import type { SchemaCatalog } from "../../../schema-catalog";
import { filterOperationTypeOptions, listOperationTypeOptions } from "../../lib/operationLabels";
import styles from "./PatchOperationTypePicker.module.css";

interface Props {
  catalog: SchemaCatalog | null;
  onSelect: (className: string) => void;
  onCancel: () => void;
}

/** Searchable list of operation types: built-in classes first, then metadata-defined custom
 * classes from the schema catalog (satisfies the "operation type selector should be searchable"
 * UX note). */
export function PatchOperationTypePicker({ catalog, onSelect, onCancel }: Props) {
  const [query, setQuery] = useState("");
  const options = useMemo(() => listOperationTypeOptions(catalog), [catalog]);
  const filtered = useMemo(() => filterOperationTypeOptions(options, query), [options, query]);

  return (
    <div className={styles.picker}>
      <input
        type="text"
        className={styles.search}
        placeholder="Search operation type…"
        value={query}
        autoFocus
        onChange={(e) => setQuery(e.target.value)}
      />
      <ul className={styles.list} role="listbox">
        {filtered.map((option) => (
          <li key={option.className}>
            <button type="button" className={styles.option} onClick={() => onSelect(option.className)}>
              <span className={styles.optionLabel}>{option.label}</span>
              <span className={styles.optionClass}>
                {option.className}
                {!option.isBuiltIn ? " · custom" : ""}
              </span>
            </button>
          </li>
        ))}
        {filtered.length === 0 && <li className={styles.empty}>No matching operation type.</li>}
      </ul>
      <button type="button" className={styles.cancel} onClick={onCancel}>
        Cancel
      </button>
    </div>
  );
}
