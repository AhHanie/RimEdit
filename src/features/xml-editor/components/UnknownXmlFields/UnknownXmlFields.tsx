import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronDown, ChevronRight } from "lucide-react";
import type { FormFieldState } from "../../types/editorForm";
import { formValueToString } from "../../hooks/useXmlFormController";
import styles from "./UnknownXmlFields.module.css";

interface Props {
  fields: FormFieldState[];
}

export function UnknownXmlFields({ fields }: Props) {
  const { t } = useTranslation("editor");
  const [expanded, setExpanded] = useState(false);

  if (fields.length === 0) return null;

  return (
    <div className={styles.root}>
      <button
        className={styles.header}
        onClick={() => setExpanded((v) => !v)}
        aria-expanded={expanded}
      >
        {expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        <span>{t("unknownXmlFields.heading", { count: fields.length })}</span>
      </button>
      {expanded && (
        <dl className={styles.list}>
          {fields.map((field) => (
            <div key={field.model.id} className={styles.row}>
              <dt className={styles.name}>{field.model.label}</dt>
              <dd className={styles.value}>
                {formValueToString(field.value) || (
                  <span className={styles.noValue}>({field.model.xmlShape})</span>
                )}
              </dd>
            </div>
          ))}
        </dl>
      )}
    </div>
  );
}
