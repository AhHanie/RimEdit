import type { AboutMetadataFields, ValidationDiagnostic } from "../../../xml-editor/types/xmlDocument";
import type { AboutEditor } from "../../hooks/useAboutEditor";
import { diagnosticsForField } from "../../lib/aboutValidationText";
import { AboutTextField } from "../AboutTextField/AboutTextField";
import { AboutStringListField } from "../AboutStringListField/AboutStringListField";
import styles from "./AboutIdentitySection.module.css";

interface Props {
  fields: AboutMetadataFields;
  diagnostics: ValidationDiagnostic[];
  readOnly: boolean;
  editor: AboutEditor;
}

export function AboutIdentitySection({ fields, diagnostics, readOnly, editor }: Props) {
  return (
    <section className={styles.section}>
      <h3 className={styles.heading}>Identity</h3>
      <AboutTextField
        label="Package ID"
        value={fields.packageId.value ?? ""}
        readOnly={readOnly}
        placeholder="yourname.modname"
        diagnostics={diagnosticsForField(diagnostics, "packageId")}
        onCommit={(v) => editor.commitScalar("packageId", v)}
      />
      <AboutTextField
        label="Name"
        value={fields.name.value ?? ""}
        readOnly={readOnly}
        diagnostics={diagnosticsForField(diagnostics, "name")}
        onCommit={(v) => editor.commitScalar("name", v)}
      />
      <AboutTextField
        label="Short Name"
        value={fields.shortName.value ?? ""}
        readOnly={readOnly}
        onCommit={(v) => editor.commitScalar("shortName", v)}
      />
      <AboutTextField
        label="Author"
        value={fields.author.value ?? ""}
        readOnly={readOnly}
        onCommit={(v) => editor.commitScalar("author", v)}
      />
      <AboutStringListField
        label="Authors"
        items={fields.authors.items}
        readOnly={readOnly}
        placeholder="Add author"
        onCommit={(items) => editor.commitList("authors", items)}
      />
      <AboutTextField
        label="Mod Version"
        value={fields.modVersion.value ?? ""}
        readOnly={readOnly}
        onCommit={(v) => editor.commitScalar("modVersion", v)}
      />
      <AboutTextField
        label="URL"
        value={fields.url.value ?? ""}
        readOnly={readOnly}
        onCommit={(v) => editor.commitScalar("url", v)}
      />
      <AboutTextField
        label="Mod Icon Path"
        value={fields.modIconPath.value ?? ""}
        readOnly={readOnly}
        onCommit={(v) => editor.commitScalar("modIconPath", v)}
      />
      <AboutTextField
        label="Description"
        value={fields.description.value ?? ""}
        readOnly={readOnly}
        multiline
        onCommit={(v) => editor.commitScalar("description", v)}
      />
    </section>
  );
}
