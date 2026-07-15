import { useTranslation } from "react-i18next";
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
  const { t } = useTranslation("editor");

  return (
    <section className={styles.section}>
      <h3 className={styles.heading}>{t("about.identity.heading")}</h3>
      <AboutTextField
        label={t("about.identity.packageId")}
        value={fields.packageId.value ?? ""}
        readOnly={readOnly}
        placeholder="yourname.modname"
        diagnostics={diagnosticsForField(diagnostics, "packageId")}
        onCommit={(v) => editor.commitScalar("packageId", v)}
      />
      <AboutTextField
        label={t("about.identity.name")}
        value={fields.name.value ?? ""}
        readOnly={readOnly}
        diagnostics={diagnosticsForField(diagnostics, "name")}
        onCommit={(v) => editor.commitScalar("name", v)}
      />
      <AboutTextField
        label={t("about.identity.shortName")}
        value={fields.shortName.value ?? ""}
        readOnly={readOnly}
        onCommit={(v) => editor.commitScalar("shortName", v)}
      />
      <AboutTextField
        label={t("about.identity.author")}
        value={fields.author.value ?? ""}
        readOnly={readOnly}
        onCommit={(v) => editor.commitScalar("author", v)}
      />
      <AboutStringListField
        label={t("about.identity.authors")}
        items={fields.authors.items}
        readOnly={readOnly}
        placeholder={t("about.identity.addAuthor")}
        onCommit={(items) => editor.commitList("authors", items)}
      />
      <AboutTextField
        label={t("about.identity.modVersion")}
        value={fields.modVersion.value ?? ""}
        readOnly={readOnly}
        onCommit={(v) => editor.commitScalar("modVersion", v)}
      />
      <AboutTextField
        label={t("about.identity.url")}
        value={fields.url.value ?? ""}
        readOnly={readOnly}
        onCommit={(v) => editor.commitScalar("url", v)}
      />
      <AboutTextField
        label={t("about.identity.modIconPath")}
        value={fields.modIconPath.value ?? ""}
        readOnly={readOnly}
        onCommit={(v) => editor.commitScalar("modIconPath", v)}
      />
      <AboutTextField
        label={t("about.identity.description")}
        value={fields.description.value ?? ""}
        readOnly={readOnly}
        multiline
        onCommit={(v) => editor.commitScalar("description", v)}
      />
    </section>
  );
}
