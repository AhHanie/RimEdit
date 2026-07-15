import { useTranslation } from "react-i18next";
import type { AboutMetadataFields, ValidationDiagnostic } from "../../../xml-editor/types/xmlDocument";
import type { AboutEditor } from "../../hooks/useAboutEditor";
import { diagnosticsForField } from "../../lib/aboutValidationText";
import { renderDiagnostic } from "../../../../i18n/diagnostics";
import { AboutTextField } from "../AboutTextField/AboutTextField";
import { AboutStringListField } from "../AboutStringListField/AboutStringListField";
import styles from "./AboutVersionSection.module.css";

interface Props {
  fields: AboutMetadataFields;
  diagnostics: ValidationDiagnostic[];
  readOnly: boolean;
  editor: AboutEditor;
}

export function AboutVersionSection({ fields, diagnostics, readOnly, editor }: Props) {
  // Two separate single-namespace hooks, not `useTranslation(["diagnostics", "editor"])` with
  // `"editor:key"`-prefixed lookups -- see `AboutDependencySection`'s `DependencyRow` doc comment.
  const { i18n } = useTranslation("diagnostics");
  const { t } = useTranslation("editor");
  const { t: tCommon } = useTranslation("common");
  const targetVersionDiagnostics = diagnosticsForField(diagnostics, "targetVersion");

  return (
    <section className={styles.section}>
      <h3 className={styles.heading}>{t("about.compatibility.heading")}</h3>
      <AboutStringListField
        label={t("about.compatibility.supportedVersions")}
        items={fields.supportedVersions.items}
        readOnly={readOnly}
        placeholder="1.6"
        onCommit={(items) => editor.commitList("supportedVersions", items)}
      />
      <AboutTextField
        label={t("about.compatibility.steamAppId")}
        value={fields.steamAppId.value ?? ""}
        readOnly={readOnly}
        onCommit={(v) => editor.commitScalar("steamAppId", v)}
      />
      {fields.targetVersion.value != null && (
        <div className={styles.obsoleteRow}>
          <div className={styles.obsoleteText}>
            <span>
              {t("about.compatibility.obsoleteTargetVersion", {
                version: fields.targetVersion.value,
              })}
            </span>
            {targetVersionDiagnostics.length > 0 && (
              <span className={styles.warning}>{renderDiagnostic(targetVersionDiagnostics[0], i18n)}</span>
            )}
          </div>
          {!readOnly && (
            <button
              type="button"
              className={styles.removeBtn}
              onClick={() => editor.commitScalar("targetVersion", "")}
            >
              {tCommon("actions.remove")}
            </button>
          )}
        </div>
      )}
    </section>
  );
}
