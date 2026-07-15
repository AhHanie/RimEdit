import { useTranslation } from "react-i18next";
import type { AboutMetadataFields } from "../../../xml-editor/types/xmlDocument";
import type { AboutEditor } from "../../hooks/useAboutEditor";
import { AboutStringListField } from "../AboutStringListField/AboutStringListField";
import styles from "./AboutLoadOrderSection.module.css";

interface Props {
  fields: AboutMetadataFields;
  readOnly: boolean;
  editor: AboutEditor;
}

export function AboutLoadOrderSection({ fields, readOnly, editor }: Props) {
  const { t } = useTranslation("editor");

  return (
    <section className={styles.section}>
      <h3 className={styles.heading}>{t("about.loadOrder.heading")}</h3>
      <AboutStringListField
        label={t("about.loadOrder.loadBefore")}
        items={fields.loadBefore.items}
        readOnly={readOnly}
        placeholder="package.id"
        onCommit={(items) => editor.commitList("loadBefore", items)}
      />
      <AboutStringListField
        label={t("about.loadOrder.loadAfter")}
        items={fields.loadAfter.items}
        readOnly={readOnly}
        placeholder="package.id"
        onCommit={(items) => editor.commitList("loadAfter", items)}
      />
      <AboutStringListField
        label={t("about.loadOrder.forceLoadBefore")}
        items={fields.forceLoadBefore.items}
        readOnly={readOnly}
        placeholder="package.id"
        onCommit={(items) => editor.commitList("forceLoadBefore", items)}
      />
      <AboutStringListField
        label={t("about.loadOrder.forceLoadAfter")}
        items={fields.forceLoadAfter.items}
        readOnly={readOnly}
        placeholder="package.id"
        onCommit={(items) => editor.commitList("forceLoadAfter", items)}
      />
      <AboutStringListField
        label={t("about.loadOrder.incompatibleWith")}
        items={fields.incompatibleWith.items}
        readOnly={readOnly}
        placeholder="package.id"
        onCommit={(items) => editor.commitList("incompatibleWith", items)}
      />
    </section>
  );
}
