import { useTranslation } from "react-i18next";
import { Plus, Trash2 } from "lucide-react";
import type { ReferenceMetadata } from "../../../schema-catalog";
import type { XmlEditorFileRef } from "../../hooks/useXmlEditorSession";
import { ReferencePicker } from "./ReferencePicker";
import listStyles from "../FormFieldControl/FormFieldControl.module.css";

interface Props {
  inputId?: string;
  items: string[];
  reference: ReferenceMetadata;
  projectId: string;
  onChangeItems: (items: string[]) => void;
  onFocus?: () => void;
  onBlur?: () => void;
  readOnly?: boolean;
  onNavigateDef?: (fileRef: XmlEditorFileRef, nodeId: number | null) => void;
}

export function ReferenceListEditor({
  inputId,
  items,
  reference,
  projectId,
  onChangeItems,
  onFocus,
  onBlur,
  readOnly,
  onNavigateDef,
}: Props) {
  const { t } = useTranslation("editor");
  function updateItem(index: number, value: string) {
    if (readOnly) return;
    onChangeItems(items.map((existing, i) => (i === index ? value : existing)));
  }

  function removeItem(index: number) {
    if (readOnly) return;
    onChangeItems(items.filter((_, i) => i !== index));
  }

  function addItem() {
    if (readOnly) return;
    onChangeItems([...items, ""]);
  }

  // `inputId` normally lands on the first row's `ReferencePicker` input (below). When there are
  // no items at all, no row exists to carry it - fall back to the container itself (issue 08,
  // Plan.md section 8: "reveal and focus" needs a DOM anchor for this field even when it
  // currently has zero entries, e.g. right after a required-but-absent field is revealed).
  // `tabIndex={-1}` makes the container programmatically focusable without joining normal Tab
  // order; never set together with the row-level id, so there is never a duplicate DOM id.
  const containerId = items.length === 0 ? inputId : undefined;

  return (
    <div className={listStyles.listEditor} role="list" id={containerId} tabIndex={containerId ? -1 : undefined}>
      {items.map((item, index) => (
        <div key={index} className={listStyles.listRow} role="listitem">
          <div style={{ flex: 1, minWidth: 0 }}>
            <ReferencePicker
              inputId={index === 0 ? inputId : undefined}
              value={item}
              reference={reference}
              projectId={projectId}
              onChange={(v) => updateItem(index, v)}
              onFocus={onFocus}
              onBlur={onBlur}
              readOnly={readOnly}
              onNavigateDef={onNavigateDef}
            />
          </div>
          {!readOnly && (
            <button
              className={listStyles.listRemove}
              onClick={() => removeItem(index)}
              aria-label={t("referencePicker.removeItem", { index: index + 1 })}
              type="button"
            >
              <Trash2 size={12} />
            </button>
          )}
        </div>
      ))}
      {!readOnly && (
        <button className={listStyles.listAdd} onClick={addItem} type="button">
          <Plus size={12} />
          {t("referencePicker.addItem")}
        </button>
      )}
    </div>
  );
}
