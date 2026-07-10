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
  return (
    <section className={styles.section}>
      <h3 className={styles.heading}>Load Order</h3>
      <AboutStringListField
        label="Load Before"
        items={fields.loadBefore.items}
        readOnly={readOnly}
        placeholder="package.id"
        onCommit={(items) => editor.commitList("loadBefore", items)}
      />
      <AboutStringListField
        label="Load After"
        items={fields.loadAfter.items}
        readOnly={readOnly}
        placeholder="package.id"
        onCommit={(items) => editor.commitList("loadAfter", items)}
      />
      <AboutStringListField
        label="Force Load Before"
        items={fields.forceLoadBefore.items}
        readOnly={readOnly}
        placeholder="package.id"
        onCommit={(items) => editor.commitList("forceLoadBefore", items)}
      />
      <AboutStringListField
        label="Force Load After"
        items={fields.forceLoadAfter.items}
        readOnly={readOnly}
        placeholder="package.id"
        onCommit={(items) => editor.commitList("forceLoadAfter", items)}
      />
      <AboutStringListField
        label="Incompatible With"
        items={fields.incompatibleWith.items}
        readOnly={readOnly}
        placeholder="package.id"
        onCommit={(items) => editor.commitList("incompatibleWith", items)}
      />
    </section>
  );
}
