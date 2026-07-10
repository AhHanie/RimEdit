import { Plus, X } from "lucide-react";
import {
  fieldSchemaToControl,
  getAllObjectFields,
  resolveObjectSchema,
} from "../../../xml-editor";
import type { ObjectFieldValue, ObjectListItemValue } from "../../../xml-editor";
import type { FieldSchema, SchemaCatalog } from "../../../schema-catalog";
import { insertAt, removeAt, replaceAt } from "../../lib/arrayUtils";
import { emptyFieldValue } from "../../lib/patchValueShape";
import styles from "./PatchValueEditor.module.css";

interface RendererProps {
  fieldName: string;
  field: FieldSchema;
  value: ObjectFieldValue;
  catalog: SchemaCatalog;
  readOnly: boolean;
  onChange: (next: ObjectFieldValue) => void;
}

/** Recursive dispatcher for a single field's `ObjectFieldValue`, keyed off the same
 * `fieldSchemaToControl` classification `xml-editor`'s Def form uses. Only the control kinds
 * `isStructurallySupportedField` allows reach here -- callers fall back to raw XML for the rest
 * (reference/typedReferenceList/color/flags/readonlyUnknown). */
export function ValueFieldRenderer({ fieldName, field, value, catalog, readOnly, onChange }: RendererProps) {
  const control = fieldSchemaToControl(fieldName, field);

  switch (control) {
    case "checkbox": {
      const checked = value.kind === "boolean" ? value.value : false;
      return (
        <label className={styles.subField}>
          <input
            type="checkbox"
            checked={checked}
            disabled={readOnly}
            onChange={(e) => onChange({ kind: "boolean", value: e.target.checked })}
          />
          <span className={styles.subLabel}>{field.label || fieldName}</span>
        </label>
      );
    }
    case "select": {
      const current = value.kind === "enum" ? value.value : "";
      const allowed = field.validationHints?.allowedValues ?? [];
      return (
        <label className={styles.subField}>
          <span className={styles.subLabel}>{field.label || fieldName}</span>
          <select value={current} disabled={readOnly} onChange={(e) => onChange({ kind: "enum", value: e.target.value })}>
            <option value="">(none)</option>
            {allowed.map((v) => (
              <option key={v} value={v}>
                {v}
              </option>
            ))}
          </select>
        </label>
      );
    }
    case "number":
    case "text": {
      const current = value.kind === "scalar" ? value.value : value.kind === "enum" ? value.value : "";
      return (
        <label className={styles.subField}>
          <span className={styles.subLabel}>{field.label || fieldName}</span>
          <input
            type="text"
            value={current}
            disabled={readOnly}
            onChange={(e) => onChange({ kind: "scalar", value: e.target.value })}
          />
        </label>
      );
    }
    case "textarea": {
      const current = value.kind === "scalar" ? value.value : "";
      return (
        <label className={styles.subField}>
          <span className={styles.subLabel}>{field.label || fieldName}</span>
          <textarea
            rows={3}
            value={current}
            disabled={readOnly}
            onChange={(e) => onChange({ kind: "scalar", value: e.target.value })}
          />
        </label>
      );
    }
    case "list": {
      const items = value.kind === "list" ? value.items : [];
      return (
        <div className={styles.subField}>
          <span className={styles.subLabel}>{field.label || fieldName}</span>
          <ScalarListEditor items={items} readOnly={readOnly} onChange={(items) => onChange({ kind: "list", items })} />
        </div>
      );
    }
    case "namedMap": {
      const entries = value.kind === "namedMap" ? value.entries : [];
      return (
        <div className={styles.subField}>
          <span className={styles.subLabel}>{field.label || fieldName}</span>
          <NamedMapEditor
            entries={entries}
            readOnly={readOnly}
            onChange={(entries) => onChange({ kind: "namedMap", entries })}
          />
        </div>
      );
    }
    case "object": {
      if (value.kind !== "object") return null;
      return (
        <fieldset className={styles.objectGroup}>
          <legend className={styles.subLabel}>{field.label || fieldName}</legend>
          <ObjectFieldsEditor value={value} catalog={catalog} readOnly={readOnly} onChange={onChange} />
        </fieldset>
      );
    }
    case "objectList": {
      if (value.kind !== "objectList") return null;
      return (
        <fieldset className={styles.objectGroup}>
          <legend className={styles.subLabel}>{field.label || fieldName}</legend>
          <ObjectListFieldEditor
            itemSchemaRef={value.itemSchemaRef}
            items={value.items}
            catalog={catalog}
            readOnly={readOnly}
            onChange={(items) => onChange({ kind: "objectList", itemSchemaRef: value.itemSchemaRef, items })}
          />
        </fieldset>
      );
    }
    default:
      return null;
  }
}

function ObjectFieldsEditor({
  value,
  catalog,
  readOnly,
  onChange,
}: {
  value: ObjectFieldValue & { kind: "object" };
  catalog: SchemaCatalog;
  readOnly: boolean;
  onChange: (next: ObjectFieldValue) => void;
}) {
  if (!value.schemaRef) {
    return <div className={styles.unsupportedNote}>Unknown object type -- edit as raw XML.</div>;
  }
  const fields = getAllObjectFields(value.schemaRef, catalog);
  return (
    <>
      {[...fields.entries()].map(([name, schema]) => {
        const fieldValue = value.fields[name] ?? emptyFieldValue(name, schema, catalog);
        return (
          <ValueFieldRenderer
            key={name}
            fieldName={name}
            field={schema}
            value={fieldValue}
            catalog={catalog}
            readOnly={readOnly}
            onChange={(next) => onChange({ ...value, fields: { ...value.fields, [name]: next } })}
          />
        );
      })}
    </>
  );
}

function ObjectListFieldEditor({
  itemSchemaRef,
  items,
  catalog,
  readOnly,
  onChange,
}: {
  itemSchemaRef: string;
  items: ObjectListItemValue[];
  catalog: SchemaCatalog;
  readOnly: boolean;
  onChange: (items: ObjectListItemValue[]) => void;
}) {
  const baseSchema = catalog.objectTypes[itemSchemaRef];
  const variantNames = baseSchema?.discriminator ? Object.keys(baseSchema.discriminator.variants) : [];
  const discriminatorAttrName = baseSchema?.discriminator?.attribute ?? "Class";

  /** Blank field set for a newly-chosen class, mirroring `buildObjectListItemValue`'s attribute
   * vs. child-element split so `attributeFields`/`fieldOrder` are populated up front -- without
   * them, `objectFieldValueToInitialElement` would serialize attribute-shaped fields (e.g. a
   * non-discriminator `xml: "attribute"` field) as XML child elements instead of attributes. */
  function blankItemFields(schemaRef: string | null) {
    const fields: Record<string, ObjectFieldValue> = {};
    const attributeFields: string[] = [];
    const fieldOrder: string[] = [];
    if (schemaRef) {
      for (const [name, schema] of getAllObjectFields(schemaRef, catalog)) {
        fieldOrder.push(name);
        if (schema.xml === "attribute") {
          if (name === discriminatorAttrName) continue;
          attributeFields.push(name);
        }
        fields[name] = emptyFieldValue(name, schema, catalog);
      }
    }
    return { fields, attributeFields, fieldOrder };
  }

  function addItem() {
    const className = variantNames[0] ?? "";
    const { schemaRef } = resolveObjectSchema(itemSchemaRef, className, catalog);
    const { fields, attributeFields, fieldOrder } = blankItemFields(schemaRef);
    onChange([
      ...items,
      { nodeId: null, className, schemaRef, fields, attributeFields, fieldOrder, initialUnknownFieldCount: 0 },
    ]);
  }

  function updateItemClass(index: number, className: string) {
    const { schemaRef } = resolveObjectSchema(itemSchemaRef, className, catalog);
    const { fields, attributeFields, fieldOrder } = blankItemFields(schemaRef);
    onChange(
      replaceAt(items, index, { ...items[index], className, schemaRef, fields, attributeFields, fieldOrder }),
    );
  }

  return (
    <div className={styles.objectList}>
      {items.map((item, index) => {
        const itemFields = item.schemaRef
          ? getAllObjectFields(item.schemaRef, catalog)
          : new Map<string, FieldSchema>();
        return (
          <div key={item.nodeId ?? `new-${index}`} className={styles.objectListItem}>
            <div className={styles.objectListItemHeader}>
              {variantNames.length > 0 ? (
                <select
                  value={item.className}
                  disabled={readOnly}
                  onChange={(e) => updateItemClass(index, e.target.value)}
                >
                  <option value="">(choose class)</option>
                  {variantNames.map((name) => (
                    <option key={name} value={name}>
                      {name}
                    </option>
                  ))}
                </select>
              ) : (
                <input
                  type="text"
                  value={item.className}
                  placeholder="Class"
                  disabled={readOnly}
                  onChange={(e) => updateItemClass(index, e.target.value)}
                />
              )}
              {!readOnly && (
                <button
                  type="button"
                  className={styles.iconBtn}
                  onClick={() => onChange(removeAt(items, index))}
                  aria-label="Remove item"
                >
                  <X size={12} />
                </button>
              )}
            </div>
            {!item.schemaRef ? (
              <div className={styles.unsupportedNote}>Unknown class -- edit as raw XML to set its fields.</div>
            ) : (
              [...itemFields.entries()].map(([name, schema]) => {
                const fieldValue = item.fields[name] ?? emptyFieldValue(name, schema, catalog);
                return (
                  <ValueFieldRenderer
                    key={name}
                    fieldName={name}
                    field={schema}
                    value={fieldValue}
                    catalog={catalog}
                    readOnly={readOnly}
                    onChange={(next) =>
                      onChange(
                        replaceAt(items, index, {
                          ...item,
                          fields: { ...item.fields, [name]: next },
                        }),
                      )
                    }
                  />
                );
              })
            )}
          </div>
        );
      })}
      {!readOnly && (
        <button type="button" className={styles.addBtn} onClick={addItem}>
          <Plus size={12} /> Add item
        </button>
      )}
    </div>
  );
}

function ScalarListEditor({
  items,
  readOnly,
  onChange,
}: {
  items: string[];
  readOnly: boolean;
  onChange: (items: string[]) => void;
}) {
  return (
    <div className={styles.list}>
      {items.map((item, i) => (
        <div key={i} className={styles.listRow}>
          <input
            type="text"
            value={item}
            disabled={readOnly}
            onChange={(e) => onChange(replaceAt(items, i, e.target.value))}
          />
          {!readOnly && (
            <button type="button" className={styles.iconBtn} onClick={() => onChange(removeAt(items, i))} aria-label="Remove item">
              <X size={12} />
            </button>
          )}
        </div>
      ))}
      {!readOnly && (
        <button type="button" className={styles.addBtn} onClick={() => onChange(insertAt(items, items.length, ""))}>
          <Plus size={12} /> Add item
        </button>
      )}
    </div>
  );
}

function NamedMapEditor({
  entries,
  readOnly,
  onChange,
}: {
  entries: { key: string; value: string }[];
  readOnly: boolean;
  onChange: (entries: { key: string; value: string }[]) => void;
}) {
  return (
    <div className={styles.list}>
      {entries.map((entry, i) => (
        <div key={i} className={styles.listRow}>
          <input
            type="text"
            value={entry.key}
            placeholder="Key"
            disabled={readOnly}
            onChange={(e) => onChange(replaceAt(entries, i, { ...entry, key: e.target.value }))}
          />
          <input
            type="text"
            value={entry.value}
            placeholder="Value"
            disabled={readOnly}
            onChange={(e) => onChange(replaceAt(entries, i, { ...entry, value: e.target.value }))}
          />
          {!readOnly && (
            <button type="button" className={styles.iconBtn} onClick={() => onChange(removeAt(entries, i))} aria-label="Remove entry">
              <X size={12} />
            </button>
          )}
        </div>
      ))}
      {!readOnly && (
        <button
          type="button"
          className={styles.addBtn}
          onClick={() => onChange(insertAt(entries, entries.length, { key: "", value: "" }))}
        >
          <Plus size={12} /> Add entry
        </button>
      )}
    </div>
  );
}
