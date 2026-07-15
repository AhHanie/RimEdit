import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { ChevronDown, ChevronRight, Plus, Trash2 } from "lucide-react";
import type {
  FormFieldState,
  FormValue,
  ObjectFieldValue,
  ObjectListItemValue,
  TypedReferenceItem,
} from "../../types/editorForm";
import type { XmlFormApi } from "../../hooks/useXmlFormController";
import { useXmlEditorContext } from "../../context/XmlEditorContext";
import type { FieldSchema, SchemaCatalog } from "../../../schema-catalog";
import {
  objectFieldValuesEqual,
  emptyValueForSchema,
} from "../../lib/formValues";
import {
  getAllObjectFields,
  resolveObjectSchema,
} from "../../lib/objectDescriptors";
import { ReferencePicker } from "../ReferencePicker/ReferencePicker";
import { ObjectFieldControl } from "./ObjectFieldControl";
import styles from "./ObjectListEditor.module.css";

const MAX_RENDER_DEPTH = 8;

// Module-level counter for stable new-item keys (reset on module reload, which is fine).
let _clientIdCounter = 1;
function nextClientId(): string {
  return `c${_clientIdCounter++}`;
}

interface Props {
  field: FormFieldState;
  formApi: XmlFormApi;
  readOnly?: boolean;
  /** DOM id for this control's root container (`fieldInputDomId(field.model.id)`, passed by
   * `FormFieldControl`). Unlike every other control kind, `ObjectListEditor` has no single
   * natural `<input>` to carry this id -- it may render zero, one, or many nested item rows.
   * Placed on the root container (with `tabIndex={-1}` so it is programmatically focusable
   * without joining normal Tab order) instead of a specific child so Form Views' "reveal and
   * focus" flow (issue 08, Plan.md section 8) has a stable, always-present target to scroll to
   * and focus regardless of how many items this field currently has. */
  id?: string;
}

export function ObjectListEditor({ field, formApi, readOnly = false, id }: Props) {
  const { catalog } = useXmlEditorContext();
  const items = field.value.kind === "objectList" ? field.value.items : [];
  const baseSchemaRef = field.model.itemSchemaRef ?? "";

  // Snapshot items once at mount - used for per-field dirty tracking and reset.
  const [initialItems] = useState<ObjectListItemValue[]>(() =>
    field.value.kind === "objectList" ? field.value.items : [],
  );

  const initialItemsByNodeId = useMemo(() => {
    const map = new Map<number, ObjectListItemValue>();
    for (const item of initialItems) {
      if (item.nodeId !== null) map.set(item.nodeId, item);
    }
    return map;
  }, [initialItems]);

  function updateItems(next: ObjectListItemValue[]) {
    if (readOnly) return;
    const nextValue: FormValue = { kind: "objectList", items: next };
    formApi.setFieldValue(field.model.id, nextValue);
  }

  function updateItem(index: number, updated: ObjectListItemValue) {
    updateItems(items.map((item, i) => (i === index ? updated : item)));
  }

  function updateItemField(
    index: number,
    fieldName: string,
    value: ObjectFieldValue,
  ) {
    updateItems(
      items.map((item, i) =>
        i === index
          ? { ...item, fields: { ...item.fields, [fieldName]: value } }
          : item,
      ),
    );
  }

  function removeItem(index: number) {
    if (readOnly) return;
    updateItems(items.filter((_, i) => i !== index));
  }

  function addItem(className: string) {
    if (readOnly) return;
    const resolvedRef = resolveSchemaRef(
      className,
      baseSchemaRef,
      catalog ?? undefined,
    );
    const effectiveOrder = catalog
      ? [...getAllObjectFields(resolvedRef ?? baseSchemaRef, catalog).keys()]
      : [];
    const newItem: ObjectListItemValue = {
      nodeId: null,
      clientId: nextClientId(),
      className,
      schemaRef: resolvedRef,
      fields: {},
      initialUnknownFieldCount: 0,
      fieldOrder: effectiveOrder.length > 0 ? effectiveOrder : undefined,
    };
    updateItems([...items, newItem]);
    formApi.focusField(field.model.id);
  }

  return (
    <div className={styles.editor} id={id} tabIndex={-1}>
      {items.map((item, index) => (
        <ObjectListItem
          key={
            item.nodeId !== null
              ? `n-${item.nodeId}`
              : (item.clientId ?? `i-${index}`)
          }
          item={item}
          index={index}
          catalog={catalog ?? undefined}
          baseSchemaRef={baseSchemaRef}
          onUpdate={(updated) => updateItem(index, updated)}
          onRemove={() => removeItem(index)}
          onFocus={() => formApi.focusField(field.model.id)}
          onBlur={() => formApi.blurField(field.model.id)}
          depth={0}
          readOnly={readOnly}
          initialFields={
            item.nodeId !== null
              ? initialItemsByNodeId.get(item.nodeId)?.fields
              : undefined
          }
          onResetField={(fieldName) => {
            if (item.nodeId === null) return;
            const orig = initialItemsByNodeId.get(item.nodeId)?.fields[
              fieldName
            ];
            if (orig !== undefined) updateItemField(index, fieldName, orig);
          }}
        />
      ))}
      {!readOnly && (
        <AddCompButton
          catalog={catalog ?? undefined}
          baseSchemaRef={baseSchemaRef}
          onAdd={addItem}
        />
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// ObjectListItem
// ---------------------------------------------------------------------------

interface ItemProps {
  item: ObjectListItemValue;
  index: number;
  catalog: SchemaCatalog | undefined;
  baseSchemaRef: string;
  onUpdate: (item: ObjectListItemValue) => void;
  onRemove: () => void;
  onFocus: () => void;
  onBlur: () => void;
  depth: number;
  readOnly?: boolean;
  initialFields?: Record<string, ObjectFieldValue>;
  onResetField?: (fieldName: string) => void;
}

function ObjectListItem({
  item,
  index,
  catalog,
  baseSchemaRef,
  onUpdate,
  onRemove,
  onFocus,
  onBlur,
  depth,
  readOnly = false,
  initialFields,
  onResetField,
}: ItemProps) {
  // Two separate single-namespace hooks, not `useTranslation(["editor", "common"])` with
  // `"common:key"`-prefixed lookups -- see `AboutDependencySection`'s `DependencyRow` doc comment.
  const { t } = useTranslation("editor");
  const { t: tCommon } = useTranslation("common");
  const [expanded, setExpanded] = useState(true);

  const hasDiscriminator = !!catalog?.objectTypes[baseSchemaRef]?.discriminator;
  const { schema, schemaRef } = catalog
    ? resolveObjectSchema(baseSchemaRef, item.className, catalog)
    : { schema: null, schemaRef: null };
  // When class is unknown, fall back to discriminator's fallbackSchemaRef (or base) so base fields still render.
  const effectiveSchemaRef =
    schemaRef ??
    (!schema && item.className && catalog
      ? (catalog.objectTypes[baseSchemaRef]?.discriminator?.fallbackSchemaRef ??
        baseSchemaRef)
      : null);
  const allFields =
    effectiveSchemaRef && catalog
      ? getAllObjectFields(effectiveSchemaRef, catalog)
      : new Map<string, FieldSchema>();
  const editableFields = [...allFields.entries()].filter(
    ([, fs]) => fs.xml !== "attribute",
  );

  const displayName = hasDiscriminator
    ? item.className
      ? prettifyClassName(item.className)
      : t("objectListEditor.noClassLabel")
    : (inferItemDisplayName(item) ?? t("objectListEditor.itemFallback", { index: index + 1 }));

  function fieldDirty(fieldName: string): boolean {
    if (!initialFields) return false;
    const init = initialFields[fieldName];
    const cur = item.fields[fieldName];
    if (init === undefined && cur === undefined) return false;
    if (init === undefined || cur === undefined) return true;
    return !objectFieldValuesEqual(init, cur);
  }

  function updateFieldValue(fieldName: string, value: ObjectFieldValue) {
    if (readOnly) return;
    onUpdate({ ...item, fields: { ...item.fields, [fieldName]: value } });
  }

  function updateClassName(newClass: string) {
    if (readOnly) return;
    const newRef = resolveSchemaRef(newClass, baseSchemaRef, catalog);
    onUpdate({ ...item, className: newClass, schemaRef: newRef });
  }

  return (
    <div className={styles.item}>
      <div className={styles.itemHeader}>
        <button
          className={styles.expandBtn}
          onClick={() => setExpanded((e) => !e)}
          type="button"
          aria-label={expanded ? t("objectListEditor.collapse") : t("objectListEditor.expand")}
        >
          {expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        </button>
        <span className={styles.itemLabel}>{displayName}</span>
        {!readOnly && (
          <button
            className={styles.removeBtn}
            onClick={onRemove}
            type="button"
            aria-label={t("objectListEditor.removeItem", { name: displayName })}
            title={tCommon("actions.remove")}
          >
            <Trash2 size={12} />
          </button>
        )}
      </div>

      {expanded && (
        <div className={styles.itemBody}>
          {/* Class attribute input - only for discriminator-based schemas */}
          {hasDiscriminator && (
            <div className={styles.fieldRow}>
              <label className={styles.fieldLabel}>{t("objectListEditor.class")}</label>
              <input
                type="text"
                className={styles.fieldInput}
                value={item.className}
                onChange={(e) => updateClassName(e.currentTarget.value)}
                onFocus={onFocus}
                onBlur={onBlur}
                spellCheck={false}
                placeholder={t("objectListEditor.classNamePlaceholder")}
                readOnly={readOnly}
                disabled={readOnly}
              />
            </div>
          )}

          {editableFields.map(([fieldName, fieldSchema]) => (
            <ObjectFieldRenderer
              key={fieldName}
              fieldName={fieldName}
              fieldSchema={fieldSchema}
              value={item.fields[fieldName]}
              onChange={(v) => updateFieldValue(fieldName, v)}
              onFocus={onFocus}
              onBlur={onBlur}
              catalog={catalog}
              depth={depth}
              readOnly={readOnly}
              dirty={fieldDirty(fieldName)}
              onReset={onResetField ? () => onResetField(fieldName) : null}
              initialValue={initialFields?.[fieldName]}
            />
          ))}

          {hasDiscriminator && !schema && item.className && (
            <p className={styles.unknownNotice}>
              {t("objectListEditor.noSchemaForClass")}
            </p>
          )}
          {item.initialUnknownFieldCount > 0 && (
            <p className={styles.unknownNotice}>
              {t("objectListEditor.unknownFieldsPreserved", {
                count: item.initialUnknownFieldCount,
              })}
            </p>
          )}
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// ObjectFieldRenderer - dispatches based on value kind / field schema
// ---------------------------------------------------------------------------

interface ObjectFieldRendererProps {
  fieldName: string;
  fieldSchema: FieldSchema;
  value: ObjectFieldValue | undefined;
  onChange: (value: ObjectFieldValue) => void;
  onFocus: () => void;
  onBlur: () => void;
  catalog: SchemaCatalog | undefined;
  depth: number;
  readOnly?: boolean;
  dirty?: boolean;
  onReset?: (() => void) | null;
  initialValue?: ObjectFieldValue;
}

function ObjectFieldRenderer({
  fieldName,
  fieldSchema,
  value,
  onChange,
  onFocus,
  onBlur,
  catalog,
  depth,
  readOnly = false,
  dirty = false,
  onReset = null,
  initialValue,
}: ObjectFieldRendererProps) {
  const { t } = useTranslation("editor");
  const label = fieldSchema.label ?? fieldName;

  if (value?.kind === "readonly") {
    return (
      <div className={styles.fieldRow}>
        <label className={styles.fieldLabel}>{label}</label>
        <span className={styles.readonlyValue}>{value.reason}</span>
      </div>
    );
  }

  // --- Nested object list ---
  const isObjectListField =
    fieldSchema.xml === "listOfLi" &&
    fieldSchema.items?.kind === "object" &&
    !!fieldSchema.items?.schemaRef;

  if (isObjectListField || value?.kind === "objectList") {
    const itemSchemaRef =
      fieldSchema.items?.schemaRef ??
      (value?.kind === "objectList" ? value.itemSchemaRef : "");
    if (!itemSchemaRef) {
      return (
        <div className={styles.fieldRow}>
          <label className={styles.fieldLabel}>{label}</label>
          <span className={styles.readonlyValue}>
            {t("objectListEditor.objectListNoSchema")}
          </span>
        </div>
      );
    }
    if (depth >= MAX_RENDER_DEPTH) {
      const count = value?.kind === "objectList" ? value.items.length : 0;
      return (
        <div className={styles.fieldRow}>
          <label className={styles.fieldLabel}>{label}</label>
          <span className={styles.readonlyValue}>
            {t("objectListEditor.itemCount", { count })}
          </span>
        </div>
      );
    }
    const nestedItems = value?.kind === "objectList" ? value.items : [];
    const nestedInitialItems =
      initialValue?.kind === "objectList" ? initialValue.items : undefined;
    return (
      <div className={styles.nestedSection}>
        <div className={styles.nestedSectionHeaderLabel}>{label}</div>
        <NestedObjectListPanel
          items={nestedItems}
          itemSchemaRef={itemSchemaRef}
          catalog={catalog}
          depth={depth + 1}
          readOnly={readOnly}
          onUpdate={(newItems) =>
            onChange({ kind: "objectList", itemSchemaRef, items: newItems })
          }
          onFocus={onFocus}
          onBlur={onBlur}
          initialItems={nestedInitialItems}
        />
      </div>
    );
  }

  // --- Nested single object ---
  const isObjectField =
    fieldSchema.type.kind === "object" &&
    (fieldSchema.xml === "object" || fieldSchema.xml === "element");

  if (isObjectField || value?.kind === "object") {
    const schemaRef =
      value?.kind === "object"
        ? value.schemaRef
        : fieldSchema.type.kind === "object"
          ? (fieldSchema.type.schemaRef ?? null)
          : null;
    if (!schemaRef || !catalog?.objectTypes[schemaRef]) {
      return (
        <div className={styles.fieldRow}>
          <label className={styles.fieldLabel}>{label}</label>
          <span className={styles.readonlyValue}>{t("objectListEditor.structuredObject")}</span>
        </div>
      );
    }
    if (depth >= MAX_RENDER_DEPTH) {
      return (
        <div className={styles.fieldRow}>
          <label className={styles.fieldLabel}>{label}</label>
          <span className={styles.readonlyValue}>{t("objectListEditor.nestedObject")}</span>
        </div>
      );
    }
    const objectValue = value?.kind === "object" ? value : null;
    const objectInitialValue =
      initialValue?.kind === "object" ? initialValue : null;
    return (
      <NestedObjectSection
        label={label}
        schemaRef={schemaRef}
        value={objectValue}
        catalog={catalog}
        depth={depth + 1}
        readOnly={readOnly}
        onChange={onChange}
        onFocus={onFocus}
        onBlur={onBlur}
        defaultCollapsed={fieldSchema.defaultCollapsed}
        initialObjectValue={objectInitialValue}
      />
    );
  }

  const emptyVal = emptyValueForSchema(fieldSchema);
  const onClear =
    !readOnly && emptyVal !== undefined ? () => onChange(emptyVal) : null;
  const effectiveOnReset = readOnly ? null : onReset;
  const error = validateObjectField(fieldName, fieldSchema, value, t);

  // --- Scalar list ---
  if (
    value?.kind === "list" ||
    (fieldSchema.xml === "listOfLi" && !isObjectListField)
  ) {
    const items = value?.kind === "list" ? value.items : [];
    return (
      <ObjectFieldControl
        fieldName={fieldName}
        fieldSchema={fieldSchema}
        dirty={dirty}
        onReset={effectiveOnReset}
        onClear={onClear}
        error={error}
      >
        <textarea
          className={styles.fieldTextarea}
          value={items.join("\n")}
          rows={Math.max(2, Math.min(items.length + 1, 6))}
          placeholder={t("objectListEditor.onePerLinePlaceholder")}
          readOnly={readOnly}
          disabled={readOnly}
          onChange={(e) =>
            onChange({
              kind: "list",
              items: e.currentTarget.value
                .split("\n")
                .filter((s) => s.trim() !== ""),
            })
          }
          onFocus={onFocus}
          onBlur={onBlur}
        />
      </ObjectFieldControl>
    );
  }

  // --- Flags ---
  if (
    value?.kind === "flags" ||
    fieldSchema.xml === "flagsText" ||
    (fieldSchema.xml === "listOfLi" && fieldSchema.flags && !isObjectListField)
  ) {
    const selected = value?.kind === "flags" ? value.selected : [];
    const custom = value?.kind === "flags" ? value.custom : [];
    const all = [...selected, ...custom];
    const allowed = fieldSchema.validationHints?.allowedValues ?? [];
    const flagsXmlShape =
      value?.kind === "flags"
        ? value.xmlShape
        : (fieldSchema.xml as "flagsText" | "listOfLi");
    return (
      <ObjectFieldControl
        fieldName={fieldName}
        fieldSchema={fieldSchema}
        dirty={dirty}
        onReset={effectiveOnReset}
        onClear={onClear}
        error={error}
      >
        <input
          type="text"
          className={styles.fieldInput}
          value={all.join(", ")}
          placeholder={t("objectListEditor.commaSeparatedFlagsPlaceholder")}
          readOnly={readOnly}
          disabled={readOnly}
          onChange={(e) => {
            const parts = e.currentTarget.value
              .split(",")
              .map((s) => s.trim())
              .filter(Boolean);
            const allowedSet = new Set(allowed);
            onChange({
              kind: "flags",
              selected: parts.filter((v) => allowedSet.has(v)),
              custom: parts.filter((v) => !allowedSet.has(v)),
              xmlShape: flagsXmlShape,
            });
          }}
          onFocus={onFocus}
          onBlur={onBlur}
        />
      </ObjectFieldControl>
    );
  }

  // --- Named map ---
  if (
    value?.kind === "namedMap" ||
    fieldSchema.xml === "namedChildrenMap" ||
    fieldSchema.xml === "keyedValueList"
  ) {
    const entries = value?.kind === "namedMap" ? value.entries : [];
    return (
      <ObjectFieldControl
        fieldName={fieldName}
        fieldSchema={fieldSchema}
        dirty={dirty}
        onReset={effectiveOnReset}
        onClear={onClear}
        error={error}
      >
        <NamedMapInlineEditor
          entries={entries}
          onChange={(e) => onChange({ kind: "namedMap", entries: e })}
          onFocus={onFocus}
          onBlur={onBlur}
          readOnly={readOnly}
        />
      </ObjectFieldControl>
    );
  }

  // --- Typed reference list ---
  if (
    value?.kind === "typedReferenceList" ||
    fieldSchema.xml === "typedReferenceList"
  ) {
    const items = value?.kind === "typedReferenceList" ? value.items : [];
    return (
      <ObjectFieldControl
        fieldName={fieldName}
        fieldSchema={fieldSchema}
        dirty={dirty}
        onReset={effectiveOnReset}
        onClear={onClear}
        error={error}
      >
        <TypedRefListInlineEditor
          items={items}
          onChange={(i) => onChange({ kind: "typedReferenceList", items: i })}
          onFocus={onFocus}
          onBlur={onBlur}
          readOnly={readOnly}
        />
      </ObjectFieldControl>
    );
  }

  // --- Scalar delegate ---
  return (
    <ObjectFieldControl
      fieldName={fieldName}
      fieldSchema={fieldSchema}
      dirty={dirty}
      onReset={effectiveOnReset}
      onClear={onClear}
      error={error}
    >
      <CompFieldInput
        fieldName={fieldName}
        fieldSchema={fieldSchema}
        value={value}
        onChange={onChange}
        onFocus={onFocus}
        onBlur={onBlur}
        readOnly={readOnly}
      />
    </ObjectFieldControl>
  );
}

// ---------------------------------------------------------------------------
// NestedObjectSection
// ---------------------------------------------------------------------------

interface NestedObjectSectionProps {
  label: string;
  schemaRef: string;
  value: (ObjectFieldValue & { kind: "object" }) | null;
  catalog: SchemaCatalog | undefined;
  depth: number;
  readOnly?: boolean;
  onChange: (v: ObjectFieldValue) => void;
  onFocus: () => void;
  onBlur: () => void;
  defaultCollapsed?: boolean;
  initialObjectValue?: (ObjectFieldValue & { kind: "object" }) | null;
}

function NestedObjectSection({
  label,
  schemaRef,
  value,
  catalog,
  depth,
  readOnly = false,
  onChange,
  onFocus,
  onBlur,
  defaultCollapsed,
  initialObjectValue,
}: NestedObjectSectionProps) {
  const { t } = useTranslation("editor");
  const schema = catalog?.objectTypes[schemaRef] ?? null;
  const allFields = catalog
    ? getAllObjectFields(schemaRef, catalog)
    : new Map<string, FieldSchema>();
  const editableFields = [...allFields.entries()].filter(
    ([, fs]) => fs.xml !== "attribute",
  );

  const currentValue: ObjectFieldValue & { kind: "object" } = value ?? {
    kind: "object",
    schemaRef,
    fields: {},
    nodeId: null,
    initialUnknownFieldCount: 0,
    fieldOrder: [...allFields.keys()],
  };

  const hasData = editableFields.some(([fieldName]) => {
    const v = currentValue.fields[fieldName];
    return (
      v && v.kind !== "readonly" && !(v.kind === "scalar" && v.value === "")
    );
  });

  // Start expanded when the schema marks it non-collapsed, or when data is present.
  const defaultExpanded = !(defaultCollapsed ?? true);
  const [expanded, setExpanded] = useState(defaultExpanded || hasData);

  function updateField(fieldName: string, fieldValue: ObjectFieldValue) {
    if (readOnly) return;
    onChange({
      ...currentValue,
      fields: { ...currentValue.fields, [fieldName]: fieldValue },
    });
  }

  function childFieldDirty(fieldName: string): boolean {
    if (!initialObjectValue) return false;
    const init = initialObjectValue.fields[fieldName];
    const cur = currentValue.fields[fieldName];
    if (!init && !cur) return false;
    if (!init || !cur) return true;
    return !objectFieldValuesEqual(init, cur);
  }

  function childOnReset(fieldName: string): (() => void) | null {
    if (readOnly) return null;
    const initField = initialObjectValue?.fields[fieldName];
    if (!initialObjectValue || initField === undefined) return null;
    return () => updateField(fieldName, initField);
  }

  return (
    <div className={styles.nestedSection}>
      <button
        className={styles.nestedSectionHeader}
        onClick={() => setExpanded((e) => !e)}
        type="button"
        aria-expanded={expanded}
      >
        {expanded ? <ChevronDown size={10} /> : <ChevronRight size={10} />}
        <span className={styles.nestedSectionHeaderLabel}>
          {label}
          {hasData && !expanded && <span className={styles.hasDataDot} />}
        </span>
      </button>

      {expanded && (
        <div className={styles.nestedSectionBody}>
          {editableFields.map(([fieldName, fieldSchema]) => (
            <ObjectFieldRenderer
              key={fieldName}
              fieldName={fieldName}
              fieldSchema={fieldSchema}
              value={currentValue.fields[fieldName]}
              onChange={(v) => updateField(fieldName, v)}
              onFocus={onFocus}
              onBlur={onBlur}
              catalog={catalog}
              depth={depth}
              readOnly={readOnly}
              dirty={childFieldDirty(fieldName)}
              onReset={childOnReset(fieldName)}
              initialValue={initialObjectValue?.fields[fieldName]}
            />
          ))}
          {!schema && (
            <p className={styles.unknownNotice}>{t("objectListEditor.unknownSchema")}</p>
          )}
          {currentValue.initialUnknownFieldCount > 0 && (
            <p className={styles.unknownNotice}>
              {t("objectListEditor.unknownFieldsPreserved", {
                count: currentValue.initialUnknownFieldCount,
              })}
            </p>
          )}
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// NestedObjectListPanel
// ---------------------------------------------------------------------------

interface NestedObjectListPanelProps {
  items: ObjectListItemValue[];
  itemSchemaRef: string;
  catalog: SchemaCatalog | undefined;
  depth: number;
  readOnly?: boolean;
  onUpdate: (items: ObjectListItemValue[]) => void;
  onFocus: () => void;
  onBlur: () => void;
  initialItems?: ObjectListItemValue[];
}

function NestedObjectListPanel({
  items,
  itemSchemaRef,
  catalog,
  depth,
  readOnly = false,
  onUpdate,
  onFocus,
  onBlur,
  initialItems,
}: NestedObjectListPanelProps) {
  const [initialItemsSnapshot] = useState<ObjectListItemValue[]>(
    () => initialItems ?? [],
  );

  const initialNestedItemsByNodeId = useMemo(() => {
    const map = new Map<number, ObjectListItemValue>();
    for (const item of initialItemsSnapshot) {
      if (item.nodeId !== null) map.set(item.nodeId, item);
    }
    return map;
  }, [initialItemsSnapshot]);

  function updateItem(index: number, updated: ObjectListItemValue) {
    if (readOnly) return;
    onUpdate(items.map((item, i) => (i === index ? updated : item)));
  }

  function removeItem(index: number) {
    if (readOnly) return;
    onUpdate(items.filter((_, i) => i !== index));
  }

  function addItem(className: string) {
    if (readOnly) return;
    const newItem: ObjectListItemValue = {
      nodeId: null,
      clientId: nextClientId(),
      className,
      schemaRef: resolveSchemaRef(className, itemSchemaRef, catalog),
      fields: {},
      initialUnknownFieldCount: 0,
    };
    onUpdate([...items, newItem]);
    onFocus();
  }

  return (
    <div className={styles.nestedList}>
      {items.map((item, index) => (
        <ObjectListItem
          key={
            item.nodeId !== null
              ? `n-${item.nodeId}`
              : (item.clientId ?? `i-${index}`)
          }
          item={item}
          index={index}
          catalog={catalog}
          baseSchemaRef={itemSchemaRef}
          onUpdate={(updated) => updateItem(index, updated)}
          onRemove={() => removeItem(index)}
          onFocus={onFocus}
          onBlur={onBlur}
          depth={depth}
          readOnly={readOnly}
          initialFields={
            item.nodeId !== null
              ? initialNestedItemsByNodeId.get(item.nodeId)?.fields
              : undefined
          }
          onResetField={(fieldName) => {
            if (item.nodeId === null) return;
            const orig = initialNestedItemsByNodeId.get(item.nodeId)?.fields[
              fieldName
            ];
            if (orig !== undefined) {
              updateItem(index, {
                ...item,
                fields: { ...item.fields, [fieldName]: orig },
              });
            }
          }}
        />
      ))}
      {!readOnly && (
        <AddCompButton
          catalog={catalog}
          baseSchemaRef={itemSchemaRef}
          onAdd={addItem}
        />
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// NamedMapInlineEditor
// ---------------------------------------------------------------------------

interface NamedMapInlineEditorProps {
  entries: { key: string; value: string }[];
  onChange: (entries: { key: string; value: string }[]) => void;
  onFocus: () => void;
  onBlur: () => void;
  readOnly?: boolean;
}

function NamedMapInlineEditor({
  entries,
  onChange,
  onFocus,
  onBlur,
  readOnly = false,
}: NamedMapInlineEditorProps) {
  // Two separate single-namespace hooks, not `useTranslation(["editor", "common"])` with
  // `"common:key"`-prefixed lookups -- see `AboutDependencySection`'s `DependencyRow` doc comment.
  const { t } = useTranslation("editor");
  const { t: tCommon } = useTranslation("common");
  function updateEntry(index: number, field: "key" | "value", val: string) {
    if (readOnly) return;
    onChange(entries.map((e, i) => (i === index ? { ...e, [field]: val } : e)));
  }

  function removeEntry(index: number) {
    if (readOnly) return;
    onChange(entries.filter((_, i) => i !== index));
  }

  function addEntry() {
    if (readOnly) return;
    onChange([...entries, { key: "", value: "" }]);
    onFocus();
  }

  return (
    <div className={styles.mapRows}>
      {entries.map((entry, index) => (
        <div key={index} className={styles.mapRow}>
          <input
            type="text"
            className={styles.mapKeyInput}
            value={entry.key}
            placeholder={t("objectListEditor.keyPlaceholder")}
            readOnly={readOnly}
            disabled={readOnly}
            onChange={(e) => updateEntry(index, "key", e.currentTarget.value)}
            onFocus={onFocus}
            onBlur={onBlur}
            spellCheck={false}
          />
          <input
            type="text"
            className={styles.mapValueInput}
            value={entry.value}
            placeholder={t("objectListEditor.valuePlaceholder")}
            readOnly={readOnly}
            disabled={readOnly}
            onChange={(e) => updateEntry(index, "value", e.currentTarget.value)}
            onFocus={onFocus}
            onBlur={onBlur}
            spellCheck={false}
          />
          {!readOnly && (
            <button
              className={styles.mapRemoveBtn}
              onClick={() => removeEntry(index)}
              type="button"
              title={tCommon("actions.remove")}
            >
              <Trash2 size={10} />
            </button>
          )}
        </div>
      ))}
      {!readOnly && (
        <button className={styles.mapAddBtn} onClick={addEntry} type="button">
          <Plus size={10} /> {tCommon("actions.add")}
        </button>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// TypedRefListInlineEditor
// ---------------------------------------------------------------------------

interface TypedRefListInlineEditorProps {
  items: TypedReferenceItem[];
  onChange: (items: TypedReferenceItem[]) => void;
  onFocus: () => void;
  onBlur: () => void;
  readOnly?: boolean;
}

function TypedRefListInlineEditor({
  items,
  onChange,
  onFocus,
  onBlur,
  readOnly = false,
}: TypedRefListInlineEditorProps) {
  // Two separate single-namespace hooks, not `useTranslation(["editor", "common"])` with
  // `"common:key"`-prefixed lookups -- see `AboutDependencySection`'s `DependencyRow` doc comment.
  const { t } = useTranslation("editor");
  const { t: tCommon } = useTranslation("common");
  function updateItem(
    index: number,
    field: "defType" | "defName",
    val: string,
  ) {
    if (readOnly) return;
    onChange(
      items.map((item, i) => (i === index ? { ...item, [field]: val } : item)),
    );
  }

  function removeItem(index: number) {
    if (readOnly) return;
    onChange(items.filter((_, i) => i !== index));
  }

  function addItem() {
    if (readOnly) return;
    onChange([...items, { nodeId: null, defType: "", defName: "" }]);
    onFocus();
  }

  return (
    <div className={styles.mapRows}>
      {items.map((item, index) => (
        <div key={item.nodeId ?? index} className={styles.mapRow}>
          <input
            type="text"
            className={styles.mapKeyInput}
            value={item.defType}
            placeholder={t("objectListEditor.defTypePlaceholder")}
            readOnly={readOnly}
            disabled={readOnly}
            onChange={(e) =>
              updateItem(index, "defType", e.currentTarget.value)
            }
            onFocus={onFocus}
            onBlur={onBlur}
            spellCheck={false}
          />
          <input
            type="text"
            className={styles.mapValueInput}
            value={item.defName}
            placeholder={t("objectListEditor.defNamePlaceholder")}
            readOnly={readOnly}
            disabled={readOnly}
            onChange={(e) =>
              updateItem(index, "defName", e.currentTarget.value)
            }
            onFocus={onFocus}
            onBlur={onBlur}
            spellCheck={false}
          />
          {!readOnly && (
            <button
              className={styles.mapRemoveBtn}
              onClick={() => removeItem(index)}
              type="button"
              title={tCommon("actions.remove")}
            >
              <Trash2 size={10} />
            </button>
          )}
        </div>
      ))}
      {!readOnly && (
        <button className={styles.mapAddBtn} onClick={addItem} type="button">
          <Plus size={10} /> {tCommon("actions.add")}
        </button>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// CompFieldInput - scalar / boolean / enum / reference inputs (no wrapper div)
// ---------------------------------------------------------------------------

interface CompFieldInputProps {
  fieldName: string;
  fieldSchema: FieldSchema;
  value: ObjectFieldValue | undefined;
  onChange: (value: ObjectFieldValue) => void;
  onFocus: () => void;
  onBlur: () => void;
  readOnly?: boolean;
}

function CompFieldInput({
  fieldName,
  fieldSchema,
  value,
  onChange,
  onFocus,
  onBlur,
  readOnly = false,
}: CompFieldInputProps) {
  const { projectId, onNavigateDef } = useXmlEditorContext();
  const kind = fieldSchema.type.kind;

  if (kind === "defReference" && fieldSchema.reference && projectId) {
    const refValue = value?.kind === "scalar" ? value.value : "";
    return (
      <ReferencePicker
        inputId={fieldName}
        value={refValue}
        reference={fieldSchema.reference}
        projectId={projectId}
        onChange={(v) => onChange({ kind: "scalar", value: v })}
        onFocus={onFocus}
        onBlur={onBlur}
        onNavigateDef={onNavigateDef}
        readOnly={readOnly}
      />
    );
  }

  if (kind === "boolean") {
    const checked = value?.kind === "boolean" ? value.value : false;
    return (
      <input
        type="checkbox"
        className={styles.fieldCheckbox}
        checked={checked}
        disabled={readOnly}
        onChange={(e) =>
          onChange({ kind: "boolean", value: e.currentTarget.checked })
        }
        onFocus={onFocus}
        onBlur={onBlur}
      />
    );
  }

  if (kind === "enum" && fieldSchema.validationHints?.allowedValues?.length) {
    const current =
      value?.kind === "enum" || value?.kind === "scalar"
        ? (value as { value: string }).value
        : "";
    return (
      <select
        className={styles.fieldSelect}
        value={current}
        disabled={readOnly}
        onChange={(e) =>
          onChange({ kind: "enum", value: e.currentTarget.value })
        }
        onFocus={onFocus}
        onBlur={onBlur}
      >
        {current &&
          !fieldSchema.validationHints.allowedValues!.includes(current) && (
            <option value={current}>{current}</option>
          )}
        {(fieldSchema.validationHints.allowedValues ?? []).map((v) => (
          <option key={v} value={v}>
            {v}
          </option>
        ))}
      </select>
    );
  }

  const inputType = kind === "integer" || kind === "float" ? "number" : "text";
  const textValue = value
    ? value.kind === "boolean"
      ? value.value
        ? "true"
        : "false"
      : ((value as { value?: string }).value ?? "")
    : "";

  return (
    <input
      type={inputType}
      className={styles.fieldInput}
      value={textValue}
      readOnly={readOnly}
      disabled={readOnly}
      onChange={(e) =>
        onChange({ kind: "scalar", value: e.currentTarget.value })
      }
      onFocus={onFocus}
      onBlur={onBlur}
      spellCheck={false}
    />
  );
}

// ---------------------------------------------------------------------------
// AddCompButton
// ---------------------------------------------------------------------------

interface AddCompButtonProps {
  catalog: SchemaCatalog | undefined;
  baseSchemaRef: string;
  onAdd: (className: string) => void;
}

function AddCompButton({ catalog, baseSchemaRef, onAdd }: AddCompButtonProps) {
  // Two separate single-namespace hooks, not `useTranslation(["editor", "common"])` with
  // `"common:key"`-prefixed lookups -- see `AboutDependencySection`'s `DependencyRow` doc comment.
  const { t } = useTranslation("editor");
  const { t: tCommon } = useTranslation("common");
  const [customClass, setCustomClass] = useState("");
  const [showCustom, setShowCustom] = useState(false);

  const baseSchema = catalog?.objectTypes[baseSchemaRef];
  const hasDiscriminator = !!baseSchema?.discriminator;
  const variants = baseSchema?.discriminator?.variants ?? {};
  const knownClasses = Object.keys(variants).sort();

  function handleAdd(className: string) {
    if (className.trim()) {
      onAdd(className.trim());
      setCustomClass("");
      setShowCustom(false);
    }
  }

  if (!hasDiscriminator) {
    return (
      <div className={styles.addSection}>
        <button
          className={styles.addBtn}
          onClick={() => onAdd("")}
          type="button"
        >
          <Plus size={12} />
          {t("objectListEditor.addItem")}
        </button>
      </div>
    );
  }

  return (
    <div className={styles.addSection}>
      {knownClasses.length > 0 && (
        <select
          className={styles.addSelect}
          value=""
          onChange={(e) => {
            if (e.currentTarget.value) handleAdd(e.currentTarget.value);
          }}
        >
          <option value="">{t("objectListEditor.addKnownType")}</option>
          {knownClasses.map((cls) => (
            <option key={cls} value={cls}>
              {prettifyClassName(cls)}
            </option>
          ))}
        </select>
      )}
      {!showCustom ? (
        <button
          className={styles.addBtn}
          onClick={() => setShowCustom(true)}
          type="button"
        >
          <Plus size={12} />
          {t("objectListEditor.addCustomType")}
        </button>
      ) : (
        <div className={styles.customRow}>
          <input
            type="text"
            className={styles.customInput}
            value={customClass}
            onChange={(e) => setCustomClass(e.currentTarget.value)}
            placeholder={t("objectListEditor.classNamePlaceholder")}
            spellCheck={false}
            autoFocus
            onKeyDown={(e) => {
              if (e.key === "Enter") handleAdd(customClass);
              if (e.key === "Escape") setShowCustom(false);
            }}
          />
          <button
            className={styles.addBtn}
            onClick={() => handleAdd(customClass)}
            type="button"
          >
            {tCommon("actions.add")}
          </button>
          <button
            className={styles.cancelBtn}
            onClick={() => setShowCustom(false)}
            type="button"
          >
            {tCommon("actions.cancel")}
          </button>
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function resolveSchemaRef(
  className: string,
  baseSchemaRef: string,
  catalog: SchemaCatalog | undefined,
): string | null {
  if (!catalog) return null;
  const baseSchema = catalog.objectTypes[baseSchemaRef];
  if (!baseSchema) return null;
  const disc = baseSchema.discriminator;
  if (!disc) return baseSchemaRef;
  if (!className) return null;
  return disc.variants[className] ?? null;
}

function prettifyClassName(className: string): string {
  return className
    .replace(/^.*\./, "")
    .replace(/^CompProperties_/, "")
    .replace(/^Rule_/, "Rule ")
    .replace(/([A-Z])/g, " $1")
    .trim();
}

const DISPLAY_NAME_FIELD_PRIORITY = [
  "label",
  "id",
  "defName",
  "verbClass",
  "defaultProjectile",
];

function inferItemDisplayName(item: ObjectListItemValue): string | null {
  for (const fieldName of DISPLAY_NAME_FIELD_PRIORITY) {
    const value = item.fields[fieldName];
    if (
      (value?.kind === "scalar" || value?.kind === "enum") &&
      value.value.trim() !== ""
    ) {
      return value.value;
    }
  }
  return null;
}

function validateObjectField(
  fieldName: string,
  fieldSchema: FieldSchema,
  value: ObjectFieldValue | undefined,
  t: TFunction<"editor">,
): string | null {
  if (fieldSchema.required && isObjectFieldEmpty(value)) {
    return t("objectListEditor.requiredError", { label: fieldSchema.label ?? fieldName });
  }
  if (
    value?.kind === "enum" &&
    fieldSchema.validationHints?.allowedValues?.length
  ) {
    if (
      value.value &&
      !fieldSchema.validationHints.allowedValues.includes(value.value)
    ) {
      return t("objectListEditor.invalidValueError", { value: value.value });
    }
  }
  return null;
}

function isObjectFieldEmpty(value: ObjectFieldValue | undefined): boolean {
  if (!value) return true;
  switch (value.kind) {
    case "scalar":
    case "enum":
      return value.value.trim() === "";
    case "boolean":
      return false;
    case "list":
      return value.items.length === 0;
    case "flags":
      return value.selected.length === 0 && value.custom.length === 0;
    case "namedMap":
      return value.entries.length === 0;
    case "typedReferenceList":
      return value.items.length === 0;
    default:
      return false;
  }
}
