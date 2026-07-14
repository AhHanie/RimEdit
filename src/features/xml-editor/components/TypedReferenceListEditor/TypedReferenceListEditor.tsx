import { useEffect, useState } from "react";
import { Plus, Trash2 } from "lucide-react";
import type { TypedReferenceMetadata } from "../../../schema-catalog";
import type { TypedReferenceItem } from "../../types/editorForm";
import type { XmlEditorFileRef } from "../../hooks/useXmlEditorSession";
import { getDefIndexFacets } from "../../../def-index/api/defIndex";
import { ReferencePicker } from "../ReferencePicker/ReferencePicker";
import listStyles from "../FormFieldControl/FormFieldControl.module.css";

interface Props {
  inputId?: string;
  items: TypedReferenceItem[];
  typedReference: TypedReferenceMetadata;
  projectId: string;
  onChangeItems: (items: TypedReferenceItem[]) => void;
  onFocus?: () => void;
  onBlur?: () => void;
  readOnly?: boolean;
  onNavigateDef?: (fileRef: XmlEditorFileRef, nodeId: number | null) => void;
}

export function TypedReferenceListEditor({
  inputId,
  items,
  typedReference,
  projectId,
  onChangeItems,
  onFocus,
  onBlur,
  readOnly,
  onNavigateDef,
}: Props) {
  const [availableDefTypes, setAvailableDefTypes] = useState<string[]>([]);

  useEffect(() => {
    getDefIndexFacets(projectId, true)
      .then((facets) => {
        const fromIndex = [...facets.defTypes]
          .sort((a, b) => {
            if (b.totalCount !== a.totalCount)
              return b.totalCount - a.totalCount;
            return a.defType.localeCompare(b.defType);
          })
          .map((f) => f.defType);
        setAvailableDefTypes(fromIndex);
      })
      .catch(() => {
        // Leave empty - user can still see existing types from XML
      });
  }, [projectId]);

  // Combine index-provided types with any types already in the XML (preserve unknown mod types).
  const existingTypes = items
    .map((i) => i.defType)
    .filter((t) => t && !availableDefTypes.includes(t));
  const defTypeOptions = [...new Set([...availableDefTypes, ...existingTypes])];

  // Default new-row def type: prefer ThingDef, otherwise first in list.
  const defaultDefType = defTypeOptions.includes("ThingDef")
    ? "ThingDef"
    : (defTypeOptions[0] ?? "ThingDef");

  function updateItem(index: number, patch: Partial<TypedReferenceItem>) {
    onChangeItems(
      items.map((item, i) => (i === index ? { ...item, ...patch } : item)),
    );
  }

  function updateDefType(index: number, defType: string) {
    // Clear defName when def type changes - old value was filtered against a different type.
    updateItem(index, { defType, defName: "", nodeId: null });
  }

  function removeItem(index: number) {
    onChangeItems(items.filter((_, i) => i !== index));
  }

  function addItem() {
    onChangeItems([
      ...items,
      { nodeId: null, defType: defaultDefType, defName: "" },
    ]);
  }

  // Same empty-list DOM-anchor fallback as `ReferenceListEditor` (issue 08, Plan.md section 8):
  // `inputId` normally lands on the first row; with zero items there is no row to carry it, so
  // fall back to the container itself. Never set together with the row-level id.
  const containerId = items.length === 0 ? inputId : undefined;

  return (
    <div className={listStyles.listEditor} role="list" id={containerId} tabIndex={containerId ? -1 : undefined}>
      {items.map((item, index) => (
        <div key={index} className={listStyles.listRow} role="listitem">
          <select
            style={{
              flexShrink: 0,
              width: "38%",
              fontFamily: "var(--font-mono)",
              fontSize: "12px",
              padding: "3px 4px",
              border: "1px solid var(--border-subtle)",
              borderRadius: "3px",
              background: "var(--surface-editor)",
              color: "var(--text-primary)",
              outline: "none",
            }}
            value={item.defType}
            onChange={(e) => updateDefType(index, e.currentTarget.value)}
            onFocus={onFocus}
            onBlur={onBlur}
            disabled={readOnly}
            aria-label={`Def type for row ${index + 1}`}
          >
            {/* Preserve current value even if absent from the index (e.g. unknown mod type). */}
            {item.defType && !defTypeOptions.includes(item.defType) && (
              <option
                value={item.defType}
                style={{
                  backgroundColor: "var(--surface-editor)",
                  color: "var(--text-primary)",
                }}
              >
                {item.defType}
              </option>
            )}
            {defTypeOptions.map((dt) => (
              <option
                key={dt}
                value={dt}
                style={{
                  backgroundColor: "var(--surface-editor)",
                  color: "var(--text-primary)",
                }}
              >
                {dt}
              </option>
            ))}
          </select>
          <div style={{ flex: 1, minWidth: 0 }}>
            <ReferencePicker
              inputId={index === 0 ? inputId : undefined}
              value={item.defName}
              reference={{
                defType: item.defType,
                allowAbstract: typedReference.allowAbstract,
                scope: typedReference.scope,
              }}
              projectId={projectId}
              onChange={(v) => updateItem(index, { defName: v, nodeId: null })}
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
              aria-label={`Remove hyperlink ${index + 1}`}
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
          Add hyperlink
        </button>
      )}
    </div>
  );
}
