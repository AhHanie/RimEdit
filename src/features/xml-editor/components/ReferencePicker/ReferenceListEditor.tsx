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

  return (
    <div className={listStyles.listEditor} role="list">
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
              aria-label={`Remove item ${index + 1}`}
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
          Add item
        </button>
      )}
    </div>
  );
}
