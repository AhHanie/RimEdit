import { useTranslation } from "react-i18next";
import type { AboutMetadataFields, AboutVersionedListEntry } from "../../../xml-editor/types/xmlDocument";
import styles from "./AboutVersionedOverridesSection.module.css";

interface Props {
  fields: AboutMetadataFields;
}

/** Read-only: versioned override maps (`descriptionsByVersion`, `loadBeforeByVersion`, etc.) use
 * dynamic per-version XML keys that the generic edit ops can't target cleanly yet -- switch to the
 * XML tab to edit them. */
export function AboutVersionedOverridesSection({ fields }: Props) {
  const { t } = useTranslation("editor");
  // Three translated-label renderers, not a `[dynamicKey, entries][]` loop: react-i18next's
  // generated `t()` overloads require a literal key from the translation-keys union, which a
  // runtime `string` loop variable can never satisfy (see docs/i18n/issues/10-formatting-rtl-and-
  // release-tooling.md's "Implementation notes" for the type error this replaced).
  const listGroups: { id: string; label: (version: string) => string; entries: AboutVersionedListEntry[] }[] = [
    {
      id: "loadBefore",
      label: (version) => t("about.versionedOverrides.loadBeforeLabel", { version }),
      entries: fields.loadBeforeByVersion,
    },
    {
      id: "loadAfter",
      label: (version) => t("about.versionedOverrides.loadAfterLabel", { version }),
      entries: fields.loadAfterByVersion,
    },
    {
      id: "incompatibleWith",
      label: (version) => t("about.versionedOverrides.incompatibleWithLabel", { version }),
      entries: fields.incompatibleWithByVersion,
    },
  ];

  const hasAny =
    fields.descriptionsByVersion.length > 0 ||
    fields.modDependenciesByVersion.length > 0 ||
    listGroups.some(({ entries }) => entries.length > 0);

  if (!hasAny) return null;

  return (
    <section className={styles.section}>
      <div className={styles.headingRow}>
        <h3 className={styles.heading}>{t("about.versionedOverrides.heading")}</h3>
        <span className={styles.readOnlyBadge}>{t("about.versionedOverrides.readOnlyBadge")}</span>
      </div>
      {fields.descriptionsByVersion.map((entry) => (
        <div key={`description-${entry.version}`} className={styles.row}>
          <span className={styles.label}>
            {t("about.versionedOverrides.descriptionLabel", { version: entry.version })}
          </span>
          <span className={styles.value}>{entry.value}</span>
        </div>
      ))}
      {listGroups.map(({ id, label, entries }) =>
        entries.map((entry) => (
          <div key={`${id}-${entry.version}`} className={styles.row}>
            <span className={styles.label}>{label(entry.version)}</span>
            <span className={styles.value}>{entry.items.join(", ") || t("about.versionedOverrides.empty")}</span>
          </div>
        )),
      )}
      {fields.modDependenciesByVersion.map((entry) => (
        <div key={`modDependencies-${entry.version}`} className={styles.row}>
          <span className={styles.label}>
            {t("about.versionedOverrides.modDependenciesLabel", { version: entry.version })}
          </span>
          <span className={styles.value}>
            {entry.dependencies.map((d) => d.packageId ?? "?").join(", ") || t("about.versionedOverrides.empty")}
          </span>
        </div>
      ))}
    </section>
  );
}
