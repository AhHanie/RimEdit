import type { AboutMetadataFields, ValidationDiagnostic } from "../../../xml-editor/types/xmlDocument";
import type { AboutEditor } from "../../hooks/useAboutEditor";
import { diagnosticsForField } from "../../lib/aboutValidationText";
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
  const targetVersionDiagnostics = diagnosticsForField(diagnostics, "targetVersion");

  return (
    <section className={styles.section}>
      <h3 className={styles.heading}>Compatibility</h3>
      <AboutStringListField
        label="Supported Versions"
        items={fields.supportedVersions.items}
        readOnly={readOnly}
        placeholder="1.6"
        onCommit={(items) => editor.commitList("supportedVersions", items)}
      />
      <AboutTextField
        label="Steam App ID"
        value={fields.steamAppId.value ?? ""}
        readOnly={readOnly}
        onCommit={(v) => editor.commitScalar("steamAppId", v)}
      />
      {fields.targetVersion.value != null && (
        <div className={styles.obsoleteRow}>
          <div className={styles.obsoleteText}>
            <span>Obsolete targetVersion: {fields.targetVersion.value}</span>
            {targetVersionDiagnostics.length > 0 && (
              <span className={styles.warning}>{targetVersionDiagnostics[0].message}</span>
            )}
          </div>
          {!readOnly && (
            <button
              type="button"
              className={styles.removeBtn}
              onClick={() => editor.commitScalar("targetVersion", "")}
            >
              Remove
            </button>
          )}
        </div>
      )}
    </section>
  );
}
