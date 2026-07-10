import type { AboutMetadataFields, AboutVersionedListEntry } from "../../../xml-editor/types/xmlDocument";
import styles from "./AboutVersionedOverridesSection.module.css";

interface Props {
  fields: AboutMetadataFields;
}

/** Read-only: versioned override maps (`descriptionsByVersion`, `loadBeforeByVersion`, etc.) use
 * dynamic per-version XML keys that the generic edit ops can't target cleanly yet -- switch to the
 * XML tab to edit them. */
export function AboutVersionedOverridesSection({ fields }: Props) {
  const listGroups: [string, AboutVersionedListEntry[]][] = [
    ["loadBefore", fields.loadBeforeByVersion],
    ["loadAfter", fields.loadAfterByVersion],
    ["incompatibleWith", fields.incompatibleWithByVersion],
  ];

  const hasAny =
    fields.descriptionsByVersion.length > 0 ||
    fields.modDependenciesByVersion.length > 0 ||
    listGroups.some(([, entries]) => entries.length > 0);

  if (!hasAny) return null;

  return (
    <section className={styles.section}>
      <div className={styles.headingRow}>
        <h3 className={styles.heading}>Versioned Overrides</h3>
        <span className={styles.readOnlyBadge}>Read-only -- edit via XML</span>
      </div>
      {fields.descriptionsByVersion.map((entry) => (
        <div key={`description-${entry.version}`} className={styles.row}>
          <span className={styles.label}>description ({entry.version})</span>
          <span className={styles.value}>{entry.value}</span>
        </div>
      ))}
      {listGroups.map(([name, entries]) =>
        entries.map((entry) => (
          <div key={`${name}-${entry.version}`} className={styles.row}>
            <span className={styles.label}>
              {name} ({entry.version})
            </span>
            <span className={styles.value}>{entry.items.join(", ") || "(empty)"}</span>
          </div>
        )),
      )}
      {fields.modDependenciesByVersion.map((entry) => (
        <div key={`modDependencies-${entry.version}`} className={styles.row}>
          <span className={styles.label}>modDependencies ({entry.version})</span>
          <span className={styles.value}>
            {entry.dependencies.map((d) => d.packageId ?? "?").join(", ") || "(empty)"}
          </span>
        </div>
      ))}
    </section>
  );
}
