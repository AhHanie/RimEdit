import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronDown, ChevronRight } from "lucide-react";
import type {
  AboutMetadataView,
  ValidationDiagnostic,
  XmlEdit,
} from "../../../xml-editor/types/xmlDocument";
import { useAboutEditor } from "../../hooks/useAboutEditor";
import { AboutIdentitySection } from "../AboutIdentitySection/AboutIdentitySection";
import { AboutVersionSection } from "../AboutVersionSection/AboutVersionSection";
import { AboutDependencySection } from "../AboutDependencySection/AboutDependencySection";
import { AboutLoadOrderSection } from "../AboutLoadOrderSection/AboutLoadOrderSection";
import { AboutVersionedOverridesSection } from "../AboutVersionedOverridesSection/AboutVersionedOverridesSection";
import styles from "./AboutEditorPane.module.css";

interface Props {
  about: AboutMetadataView;
  diagnostics: ValidationDiagnostic[];
  readOnly: boolean;
  locationName?: string;
  applyFormEdit: (edit: XmlEdit) => Promise<string>;
  registerFlush?: (flush: () => Promise<void>) => void;
}

export function AboutEditorPane({
  about,
  diagnostics,
  readOnly,
  locationName,
  applyFormEdit,
  registerFlush,
}: Props) {
  const { t } = useTranslation("editor");
  const rootRef = useRef<HTMLDivElement>(null);
  const editor = useAboutEditor(
    applyFormEdit,
    about.rootNodeId,
    rootRef,
    readOnly ? undefined : registerFlush,
  );
  const fields = about.fields;
  const supportedCount = fields.supportedVersions.items.length;

  return (
    <div className={styles.root} ref={rootRef}>
      <div className={styles.header}>
        <span className={styles.modName}>{fields.name.value || t("about.pane.unnamedMod")}</span>
        <span className={styles.packageId}>{fields.packageId.value || t("about.pane.noPackageId")}</span>
        <span className={styles.supportBadge}>
          {supportedCount > 0
            ? t("about.pane.supportedVersionCount", { count: supportedCount })
            : t("about.pane.noSupportedVersions")}
        </span>
        {readOnly && (
          <span className={styles.readOnlyBadge}>
            {locationName
              ? t("about.pane.readOnlyBadgeWithLocation", { locationName })
              : t("about.pane.readOnlyBadge")}
          </span>
        )}
      </div>
      <div className={styles.scroll}>
        <AboutIdentitySection fields={fields} diagnostics={diagnostics} readOnly={readOnly} editor={editor} />
        <AboutVersionSection fields={fields} diagnostics={diagnostics} readOnly={readOnly} editor={editor} />
        <AboutDependencySection
          dependencies={fields.modDependencies}
          diagnostics={diagnostics}
          readOnly={readOnly}
          editor={editor}
        />
        <AboutLoadOrderSection fields={fields} readOnly={readOnly} editor={editor} />
        <AboutVersionedOverridesSection fields={fields} />
        {about.unknownChildren.length > 0 && <UnknownAboutFields items={about.unknownChildren} />}
      </div>
    </div>
  );
}

function UnknownAboutFields({ items }: { items: AboutMetadataView["unknownChildren"] }) {
  const [expanded, setExpanded] = useState(false);
  const { t } = useTranslation("editor");
  return (
    <div className={styles.unknownSection}>
      <button className={styles.unknownHeader} onClick={() => setExpanded((v) => !v)} aria-expanded={expanded}>
        {expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        <span>{t("about.pane.unknownElements", { count: items.length })}</span>
      </button>
      {expanded && (
        <ul className={styles.unknownList}>
          {items.map((item) => (
            <li key={item.nodeId} className={styles.unknownItem}>
              {`<${item.name}>`}
              {item.line != null && (
                <span className={styles.unknownLine}>
                  {t("about.pane.unknownElementLine", { line: item.line })}
                </span>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
