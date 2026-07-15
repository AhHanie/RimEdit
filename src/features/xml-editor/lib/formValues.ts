import type { FormFieldModel, FormValue, ObjectFieldValue, ObjectListItemValue } from "../types/editorForm";
import type { XmlEdit, XmlInitialElement } from "../types/xmlDocument";
import type { FieldSchema } from "../../schema-catalog";
import { initI18n } from "../../../i18n";

/**
 * Pure form-value helpers and per-field validation/edit derivation.
 *
 * These live in their own module (rather than inside `useXmlFormController`) so the
 * subscribable form-field store can depend on them without creating an import cycle
 * with the controller. `useXmlFormController` re-exports the public factory helpers
 * for backwards compatibility with existing component imports.
 *
 * `validateFieldValue`/`formValueToString`'s user-facing strings below are built by plain module
 * functions, not React components, so there is no `useTranslation()` hook to call -- resolves
 * translated text from the app-wide i18next singleton instead, same as
 * `src/features/xml-editor/lib/objectDescriptors.ts` (see that module's top-of-file doc comment
 * for the full rationale, including why `initI18n().t(...)` is called directly at each site
 * rather than through a same-signature local wrapper).
 */

/** The minimal field shape `fieldToXmlEdit` needs (a structural subset of the store's StoredFieldState). */
export interface FieldEditInput {
  model: FormFieldModel;
  value: FormValue;
  initialValue: FormValue;
  clearRequested?: boolean;
}

export function formValuesEqual(a: FormValue, b: FormValue): boolean {
  if (a.kind !== b.kind) return false;
  if (a.kind === "list" && b.kind === "list") {
    return a.items.length === b.items.length && a.items.every((item, index) => item === b.items[index]);
  }
  if (a.kind === "flags" && b.kind === "flags") {
    const aAll = [...a.selected, ...a.custom];
    const bAll = [...b.selected, ...b.custom];
    return aAll.length === bAll.length && aAll.every((item, index) => item === bAll[index]);
  }
  if (a.kind === "namedMap" && b.kind === "namedMap") {
    if (a.entries.length !== b.entries.length) return false;
    return a.entries.every(
      (entry, index) => entry.key === b.entries[index].key && entry.value === b.entries[index].value,
    );
  }
  if (a.kind === "typedReferenceList" && b.kind === "typedReferenceList") {
    if (a.items.length !== b.items.length) return false;
    return a.items.every(
      (item, index) =>
        item.defType === b.items[index].defType && item.defName === b.items[index].defName,
    );
  }
  if (a.kind === "objectList" && b.kind === "objectList") {
    if (a.items.length !== b.items.length) return false;
    return a.items.every((aItem, index) => {
      const bItem = b.items[index];
      if (aItem.nodeId !== bItem.nodeId) return false;
      if (aItem.className !== bItem.className) return false;
      const aFieldNames = Object.keys(aItem.fields);
      const bFieldNames = Object.keys(bItem.fields);
      if (aFieldNames.length !== bFieldNames.length) return false;
      return aFieldNames.every((name) => {
        const af = aItem.fields[name];
        const bf = bItem.fields[name];
        if (!af || !bf) return false;
        return objectFieldValuesEqual(af, bf);
      });
    });
  }
  return formValueToString(a) === formValueToString(b);
}

export function isValidXmlName(name: string): boolean {
  // XML name start char: letter, underscore, colon
  // XML name char: letter, digit, dot, hyphen, underscore, colon, combining chars, extenders
  // This is a reasonable approximation for RimWorld shader parameter names (e.g. _MyParam).
  return /^[a-zA-Z_:][a-zA-Z0-9._:\-]*$/.test(name);
}

export function formValueToString(value: FormValue): string {
  switch (value.kind) {
    case "boolean":
      return value.value ? "true" : "false";
    case "enum":
    case "readonly":
    case "scalar":
      return value.value;
    case "list":
      return value.items.join("\n");
    case "flags":
      return [...value.selected, ...value.custom].join("\n");
    case "namedMap":
      return value.entries.map((e) => `${e.key}=${e.value}`).join("\n");
    case "typedReferenceList":
      return value.items.map((i) => `${i.defType}:${i.defName}`).join("\n");
    case "objectList": {
      const count = value.items.length;
      return initI18n().t(
        "editor:objectListEditor.itemCountParens",
        `(${count} item${count === 1 ? "" : "s"})`,
        { count },
      );
    }
  }
}

export function cloneFormValue(value: FormValue): FormValue {
  if (value.kind === "list") return { kind: "list", items: [...value.items] };
  if (value.kind === "flags") {
    return { kind: "flags", selected: [...value.selected], custom: [...value.custom] };
  }
  if (value.kind === "namedMap") {
    return { kind: "namedMap", entries: value.entries.map((e) => ({ ...e })) };
  }
  if (value.kind === "typedReferenceList") {
    return { kind: "typedReferenceList", items: value.items.map((i) => ({ ...i })) };
  }
  if (value.kind === "objectList") {
    return {
      kind: "objectList",
      items: value.items.map((item) => ({
        ...item,
        fields: Object.fromEntries(
          Object.entries(item.fields).map(([k, v]) => [k, cloneObjectFieldValue(v)]),
        ),
      })),
    };
  }
  return { ...value };
}

export function scalarFormValue(value: string): FormValue {
  return { kind: "scalar", value };
}

export function listFormValue(items: string[]): FormValue {
  return { kind: "list", items: [...items] };
}

export function booleanFormValue(value: boolean): FormValue {
  return { kind: "boolean", value };
}

export function enumFormValue(value: string): FormValue {
  return { kind: "enum", value };
}

export function flagsFormValue(selected: string[], custom: string[] = []): FormValue {
  return { kind: "flags", selected: [...selected], custom: [...custom] };
}

export function namedMapFormValue(entries: { key: string; value: string }[]): FormValue {
  return { kind: "namedMap", entries: entries.map((e) => ({ ...e })) };
}

export function typedReferenceListFormValue(
  items: { nodeId: number | null; defType: string; defName: string }[],
): FormValue {
  return { kind: "typedReferenceList", items: items.map((i) => ({ ...i })) };
}

export function emptyFormValueForModel(model: FormFieldModel): FormValue {
  switch (model.control) {
    case "checkbox":
      return { kind: "boolean", value: false };
    case "select":
      return { kind: "enum", value: "" };
    case "list":
      return { kind: "list", items: [] };
    case "flags":
      return { kind: "flags", selected: [], custom: [] };
    case "namedMap":
      return { kind: "namedMap", entries: [] };
    case "typedReferenceList":
      return { kind: "typedReferenceList", items: [] };
    case "objectList":
      return { kind: "objectList", items: [] };
    default:
      return { kind: "scalar", value: "" };
  }
}

export function validateFieldValue(model: FormFieldModel, value: FormValue): string[] {
  if (model.readonly) return [];
  const errors: string[] = [];
  const label = model.label;
  if (model.required) {
    if (
      (value.kind === "list" && value.items.length === 0) ||
      (value.kind === "flags" && value.selected.length === 0 && value.custom.length === 0) ||
      (value.kind === "namedMap" && value.entries.length === 0) ||
      (value.kind === "typedReferenceList" && value.items.length === 0)
    ) {
      errors.push(
        initI18n().t("editor:formValidation.required", `${label} is required.`, { label }),
      );
    } else if (
      value.kind !== "list" &&
      value.kind !== "flags" &&
      value.kind !== "namedMap" &&
      value.kind !== "typedReferenceList" &&
      formValueToString(value).trim() === ""
    ) {
      errors.push(
        initI18n().t("editor:formValidation.required", `${label} is required.`, { label }),
      );
    }
  }
  if (value.kind === "typedReferenceList") {
    for (const item of value.items) {
      if (!item.defType.trim()) {
        errors.push(
          initI18n().t(
            "editor:formValidation.typedRefDefTypeEmpty",
            `${label}: def type cannot be empty.`,
            { label },
          ),
        );
        break;
      }
      if (!isValidXmlName(item.defType)) {
        errors.push(
          initI18n().t(
            "editor:formValidation.invalidXmlElementName",
            `${label}: "${item.defType}" is not a valid XML element name.`,
            { label, value: item.defType },
          ),
        );
        break;
      }
      if (!item.defName.trim()) {
        errors.push(
          initI18n().t(
            "editor:formValidation.typedRefDefNameEmpty",
            `${label}: def name cannot be empty.`,
            { label },
          ),
        );
        break;
      }
    }
  }
  if (value.kind === "namedMap") {
    const keys = value.entries.map((e) => e.key);
    const emptyKey = keys.find((k) => k.trim() === "");
    if (emptyKey !== undefined) {
      errors.push(
        initI18n().t(
          "editor:formValidation.namedMapKeysRequired",
          `${label}: all keys must be non-empty.`,
          { label },
        ),
      );
    }
    if (!model.repeatable) {
      const seen = new Set<string>();
      const duplicateKey = keys.find((k) => { if (seen.has(k)) return true; seen.add(k); return false; });
      if (duplicateKey !== undefined) {
        errors.push(
          initI18n().t(
            "editor:formValidation.namedMapDuplicateKey",
            `${label}: duplicate key "${duplicateKey}".`,
            { label, key: duplicateKey },
          ),
        );
      }
    }
    const invalidKey = keys.find((k) => k.trim() !== "" && !isValidXmlName(k));
    if (invalidKey !== undefined) {
      errors.push(
        initI18n().t(
          "editor:formValidation.invalidXmlElementName",
          `${label}: "${invalidKey}" is not a valid XML element name.`,
          { label, value: invalidKey },
        ),
      );
    }
  }
  if (model.allowedValues?.length && value.kind === "enum" && value.value) {
    if (!model.allowedValues.includes(value.value)) {
      errors.push(
        initI18n().t(
          "editor:formValidation.mustBeAllowedValue",
          `${label} must be one of the allowed values.`,
          { label },
        ),
      );
    }
  }
  const textValue = formValueToString(value);
  if (model.validationHints?.pattern && textValue) {
    try {
      if (!new RegExp(model.validationHints.pattern).test(textValue)) {
        errors.push(
          initI18n().t(
            "editor:formValidation.patternMismatch",
            `${label} does not match the expected format.`,
            { label },
          ),
        );
      }
    } catch {
      // Ignore invalid schema-provided patterns; schema diagnostics should cover them.
    }
  }
  if (
    (model.validationHints?.min !== undefined || model.validationHints?.max !== undefined) &&
    textValue
  ) {
    const numericValue = Number(textValue);
    if (!Number.isNaN(numericValue)) {
      const min = model.validationHints.min;
      const max = model.validationHints.max;
      if (min !== undefined && numericValue < min) {
        errors.push(
          initI18n().t("editor:formValidation.minValue", `${label} must be at least ${min}.`, {
            label,
            min,
          }),
        );
      }
      if (max !== undefined && numericValue > max) {
        errors.push(
          initI18n().t("editor:formValidation.maxValue", `${label} must be at most ${max}.`, {
            label,
            max,
          }),
        );
      }
    }
  }
  return errors;
}


/** Deep equality for recursive ObjectFieldValue. */
export function objectFieldValuesEqual(a: ObjectFieldValue, b: ObjectFieldValue): boolean {
  if (a.kind !== b.kind) return false;
  switch (a.kind) {
    case "scalar":
    case "enum":
      return a.value === (b as typeof a).value;
    case "boolean":
      return a.value === (b as typeof a).value;
    case "readonly":
      return true;
    case "list": {
      const bl = b as typeof a;
      return a.items.length === bl.items.length && a.items.every((x, i) => x === bl.items[i]);
    }
    case "flags": {
      const bf = b as typeof a;
      const aAll = [...a.selected, ...a.custom];
      const bAll = [...bf.selected, ...bf.custom];
      return aAll.length === bAll.length && aAll.every((x, i) => x === bAll[i]);
    }
    case "namedMap": {
      const bm = b as typeof a;
      if (a.entries.length !== bm.entries.length) return false;
      return a.entries.every((e, i) => e.key === bm.entries[i].key && e.value === bm.entries[i].value);
    }
    case "typedReferenceList": {
      const br = b as typeof a;
      if (a.items.length !== br.items.length) return false;
      return a.items.every((x, i) => x.defType === br.items[i].defType && x.defName === br.items[i].defName);
    }
    case "object": {
      const bo = b as typeof a;
      if (a.schemaRef !== bo.schemaRef) return false;
      const aKeys = Object.keys(a.fields);
      if (aKeys.length !== Object.keys(bo.fields).length) return false;
      return aKeys.every((k) => {
        const af = a.fields[k], bf = bo.fields[k];
        return !!af && !!bf && objectFieldValuesEqual(af, bf);
      });
    }
    case "objectList": {
      const bol = b as typeof a;
      if (a.items.length !== bol.items.length) return false;
      return a.items.every((ai, i) => {
        const bi = bol.items[i];
        if (ai.nodeId !== bi.nodeId || ai.className !== bi.className) return false;
        const aKeys = Object.keys(ai.fields);
        if (aKeys.length !== Object.keys(bi.fields).length) return false;
        return aKeys.every((k) => {
          const af = ai.fields[k], bf = bi.fields[k];
          return !!af && !!bf && objectFieldValuesEqual(af, bf);
        });
      });
    }
    default:
      return false;
  }
}

/** Deep clone for recursive ObjectFieldValue. */
export function cloneObjectFieldValue(v: ObjectFieldValue): ObjectFieldValue {
  switch (v.kind) {
    case "list": return { kind: "list", items: [...v.items] };
    case "flags": return { kind: "flags", selected: [...v.selected], custom: [...v.custom], xmlShape: v.xmlShape };
    case "namedMap": return { kind: "namedMap", entries: v.entries.map((e) => ({ ...e })) };
    case "typedReferenceList": return { kind: "typedReferenceList", items: v.items.map((i) => ({ ...i })) };
    case "object":
      return {
        kind: "object",
        schemaRef: v.schemaRef,
        fields: Object.fromEntries(Object.entries(v.fields).map(([k, fv]) => [k, cloneObjectFieldValue(fv)])),
        nodeId: v.nodeId,
        initialUnknownFieldCount: v.initialUnknownFieldCount,
        fieldXmlNames: v.fieldXmlNames ? { ...v.fieldXmlNames } : undefined,
        fieldOrder: [...v.fieldOrder],
        xmlAttributeFields: v.xmlAttributeFields ? [...v.xmlAttributeFields] : undefined,
      };
    case "objectList":
      return {
        kind: "objectList",
        itemSchemaRef: v.itemSchemaRef,
        items: v.items.map((item) => ({
          ...item,
          fields: Object.fromEntries(Object.entries(item.fields).map(([k, fv]) => [k, cloneObjectFieldValue(fv)])),
        })),
      };
    default:
      return { ...v };
  }
}

/** Returns an empty ObjectFieldValue for the given schema, or undefined for non-clearable structural types. */
export function emptyValueForSchema(fieldSchema: FieldSchema): ObjectFieldValue | undefined {
  if (fieldSchema.xml === "typedReferenceList") return { kind: "typedReferenceList", items: [] };
  if (fieldSchema.xml === "namedChildrenMap" || fieldSchema.xml === "keyedValueList") return { kind: "namedMap", entries: [] };
  if (fieldSchema.xml === "flagsText") return { kind: "flags", selected: [], custom: [], xmlShape: "flagsText" };
  if (fieldSchema.xml === "keyedObjectList") return undefined;
  if (fieldSchema.xml === "listOfLi") {
    if (fieldSchema.items?.kind === "object") return undefined;
    if (fieldSchema.flags) return { kind: "flags", selected: [], custom: [], xmlShape: "listOfLi" };
    return { kind: "list", items: [] };
  }
  const kind = fieldSchema.type.kind;
  if (kind === "boolean") return { kind: "boolean", value: false };
  if (kind === "enum") return { kind: "enum", value: "" };
  if (kind === "object") return undefined;
  return { kind: "scalar", value: "" };
}

/**
 * Serialize an ObjectFieldValue to a text string for scalar-like edits.
 * Returns empty string for structural values (object, objectList) which
 * are handled by their own recursive edit paths.
 */
function objectFieldValueToText(v: ObjectFieldValue): string {
  switch (v.kind) {
    case "scalar":
    case "enum":
      return v.value;
    case "boolean":
      return v.value ? "true" : "false";
    case "list":
      return v.items.join("\n");
    case "flags":
      return [...v.selected, ...v.custom].join(", ");
    default:
      return "";
  }
}

// ---------------------------------------------------------------------------
// Recursive initial-element tree builders (for new object-list items)
// ---------------------------------------------------------------------------

/**
 * Build a list of XmlInitialElement nodes for all non-empty fields in `fields`.
 * Uses `fieldOrder` for ordering when provided; falls back to Object.keys order.
 */
function buildInitialChildren(
  fields: Record<string, ObjectFieldValue>,
  fieldXmlNames?: Record<string, string>,
  fieldOrder?: string[],
  skipFields?: readonly string[],
): XmlInitialElement[] {
  const result: XmlInitialElement[] = [];
  const orderedKeys = fieldOrder
    ? [
        ...fieldOrder.filter((k) => k in fields),
        ...Object.keys(fields).filter((k) => !fieldOrder.includes(k)),
      ]
    : Object.keys(fields);
  for (const fieldName of orderedKeys) {
    if (skipFields?.includes(fieldName)) continue;
    const v = fields[fieldName];
    if (!v || v.kind === "readonly") continue;
    const xmlName = fieldXmlNames?.[fieldName] ?? fieldName;
    const elem = objectFieldValueToInitialElement(xmlName, v);
    if (elem) result.push(elem);
  }
  return result;
}

/** Exported for `patches-editor`'s `PatchValueEditor` (issue 06), which reuses this pure,
 * node-id-free builder to turn an edited `ObjectFieldValue` into an `XmlInitialElement` fragment
 * for a patch operation's `<value>` payload -- the same tree shape object-list item insertion
 * already sends over IPC, just serialized to a standalone XML string instead of applied as a
 * document edit. */
export function objectFieldValueToInitialElement(
  xmlName: string,
  v: ObjectFieldValue,
): XmlInitialElement | null {
  switch (v.kind) {
    case "scalar":
      return v.value ? { name: xmlName, value: v.value } : null;
    case "boolean":
      return { name: xmlName, value: v.value ? "true" : "false" };
    case "enum":
      return v.value ? { name: xmlName, value: v.value } : null;
    case "list":
      if (!v.items.length) return null;
      return { name: xmlName, liItems: v.items.map((i) => ({ name: "li", value: i })) };
    case "flags": {
      const all = [...v.selected, ...v.custom];
      if (!all.length) return null;
      if (v.xmlShape === "listOfLi") {
        return { name: xmlName, liItems: all.map((i) => ({ name: "li", value: i })) };
      }
      return { name: xmlName, value: all.join(", ") };
    }
    case "namedMap":
      if (!v.entries.length) return null;
      return { name: xmlName, children: v.entries.map((e) => ({ name: e.key, value: e.value })) };
    case "typedReferenceList":
      if (!v.items.length) return null;
      return { name: xmlName, children: v.items.map((i) => ({ name: i.defType, value: i.defName })) };
    case "object": {
      const attrFields = v.xmlAttributeFields;
      const attrs: { name: string; value: string }[] = [];
      for (const fieldName of attrFields ?? []) {
        const fv = v.fields[fieldName];
        if (!fv || fv.kind === "readonly") continue;
        const attrXmlName = v.fieldXmlNames?.[fieldName] ?? fieldName;
        const text = objectFieldValueToText(fv);
        if (text) attrs.push({ name: attrXmlName, value: text });
      }
      const children = buildInitialChildren(v.fields, v.fieldXmlNames, v.fieldOrder, attrFields);
      if (!children.length && !attrs.length) return null;
      return {
        name: xmlName,
        attributes: attrs.length ? attrs : undefined,
        children: children.length ? children : undefined,
      };
    }
    case "objectList": {
      if (!v.items.length) return null;
      return {
        name: xmlName,
        liItems: v.items.map(buildInitialListItemElement),
      };
    }
    case "readonly":
      return null;
  }
}

function buildInitialListItemElement(item: ObjectListItemValue): XmlInitialElement {
  const attrs: { name: string; value: string }[] = [];
  if (item.className) attrs.push({ name: "Class", value: item.className });
  for (const fieldName of item.attributeFields ?? []) {
    const v = item.fields[fieldName];
    if (!v || v.kind === "readonly") continue;
    const xmlName = item.fieldXmlNames?.[fieldName] ?? fieldName;
    const text = objectFieldValueToText(v);
    if (text) attrs.push({ name: xmlName, value: text });
  }
  const children = buildInitialChildren(item.fields, item.fieldXmlNames, item.fieldOrder, item.attributeFields);
  return {
    name: "li",
    attributes: attrs.length ? attrs : undefined,
    children: children.length ? children : undefined,
  };
}

/**
 * Recursively diff an ObjectFieldValue against its initial counterpart and emit
 * XmlEdits anchored at `itemNodeId`. `objectPath` is the path of nested object
 * elements within the item up to (but not including) the current field, and
 * `xmlName` is the current field's XML element name.
 *
 * For direct children of the list item (objectPath=[]), scalar edits use the
 * setObjectListItemChildText / removeObjectListItemChild operations. For deeper
 * fields, setNestedElementText / removeNestedElement are used with
 * parentNodeId=itemNodeId. `parentFieldOrder` is the fieldOrder of the containing
 * object schema and is included on edits so Rust can insert fields in schema order.
 */
function diffObjectFieldValue(
  current: ObjectFieldValue,
  initial: ObjectFieldValue | undefined,
  itemNodeId: number,
  objectPath: string[],
  xmlName: string,
  parentFieldOrder?: string[],
): XmlEdit[] {
  if (current.kind === "readonly") return [];

  // Nested single object: recurse into its fields, extending the objectPath.
  if (current.kind === "object") {
    if (initial !== undefined && initial.kind !== "object") return [];
    const nestedPath = [...objectPath, xmlName]; // path from itemNodeId to this object element
    const edits: XmlEdit[] = [];
    for (const [fieldName, fieldValue] of Object.entries(current.fields)) {
      if (fieldValue.kind === "readonly") continue;
      const actualXml = current.fieldXmlNames?.[fieldName] ?? fieldName;
      const initialField = initial?.kind === "object" ? initial.fields[fieldName] : undefined;

      if (current.xmlAttributeFields?.includes(fieldName)) {
        // Attribute on this object element - do not recurse, emit attribute edit directly.
        const curText = objectFieldValueToText(fieldValue);
        const initText = initialField !== undefined ? objectFieldValueToText(initialField) : null;
        if (curText === initText) continue;
        if (curText) {
          // Use setElementAttribute when the element already exists; otherwise create the path.
          if (current.nodeId !== null) {
            edits.push({ type: "setElementAttribute", elementNodeId: current.nodeId, attributeName: actualXml, value: curText });
          } else {
            edits.push({ type: "setNestedElementAttribute", parentNodeId: itemNodeId, objectPath: nestedPath, attributeName: actualXml, value: curText });
          }
        } else if (current.nodeId !== null) {
          edits.push({ type: "removeElementAttribute", elementNodeId: current.nodeId, attributeName: actualXml });
        }
        continue;
      }

      edits.push(...diffObjectFieldValue(
        fieldValue,
        initialField,
        itemNodeId,
        nestedPath,
        actualXml,
        current.fieldOrder,
      ));
    }
    return edits;
  }

  // Nested object list: diff items by nodeId.
  if (current.kind === "objectList") {
    const initialItems: ObjectListItemValue[] =
      initial?.kind === "objectList" ? initial.items : [];
    const edits: XmlEdit[] = [];
    const listObjectPath = objectPath.length > 0 ? objectPath : undefined;

    // Removed items
    const currentNodeIds = new Set(
      current.items.filter((i) => i.nodeId !== null).map((i) => i.nodeId!),
    );
    for (const initItem of initialItems) {
      if (initItem.nodeId !== null && !currentNodeIds.has(initItem.nodeId)) {
        edits.push({ type: "removeObjectListItem", listItemNodeId: initItem.nodeId, pruneEmptyAncestors: true });
      }
    }

    // New and existing items
    const initialByNodeId = new Map(
      initialItems.filter((i) => i.nodeId !== null).map((i) => [i.nodeId!, i]),
    );
    for (const item of current.items) {
      if (item.nodeId === null) {
        // New nested item - build full recursive initial tree.
        const initialChildren = buildInitialChildren(item.fields, item.fieldXmlNames);
        edits.push({
          type: "insertObjectListItem",
          parentNodeId: itemNodeId,
          objectPath: listObjectPath,
          listName: xmlName,
          classAttribute: item.className || undefined,
          initialChildren: initialChildren.length > 0 ? initialChildren : undefined,
        });
      } else {
        const initItem = initialByNodeId.get(item.nodeId);
        if (initItem && item.className !== initItem.className) {
          edits.push({
            type: "setObjectListItemAttribute",
            listItemNodeId: item.nodeId,
            attributeName: "Class",
            value: item.className,
          });
        }
        if (initItem) {
          for (const [fieldName, fieldValue] of Object.entries(item.fields)) {
            if (fieldValue.kind === "readonly") continue;
            const actualXml = item.fieldXmlNames?.[fieldName] ?? fieldName;
            const initField = initItem.fields[fieldName];
            if (item.attributeFields?.includes(fieldName)) {
              // Attribute on the <li> element itself.
              const curText = objectFieldValueToText(fieldValue);
              const initText = initField !== undefined ? objectFieldValueToText(initField) : null;
              if (curText !== initText) {
                if (curText) {
                  edits.push({ type: "setObjectListItemAttribute", listItemNodeId: item.nodeId, attributeName: actualXml, value: curText });
                } else {
                  edits.push({ type: "removeElementAttribute", elementNodeId: item.nodeId, attributeName: actualXml });
                }
              }
            } else {
              edits.push(...diffObjectFieldValue(fieldValue, initField, item.nodeId, [], actualXml, item.fieldOrder));
            }
          }
        }
      }
    }
    return edits;
  }

  // List (scalar li list)
  if (current.kind === "list") {
    const initItems = initial?.kind === "list" ? initial.items : [];
    if (current.items.length === initItems.length && current.items.every((x, i) => x === initItems[i])) {
      return [];
    }
    return [{
      type: "setNestedListItems",
      parentNodeId: itemNodeId,
      objectPath,
      fieldName: xmlName,
      items: current.items,
      ...(parentFieldOrder?.length ? { fieldOrder: parentFieldOrder } : {}),
    }];
  }

  // Flags
  if (current.kind === "flags") {
    const initAll = initial?.kind === "flags" ? [...initial.selected, ...initial.custom] : [];
    const curAll = [...current.selected, ...current.custom];
    if (curAll.length === initAll.length && curAll.every((x, i) => x === initAll[i])) {
      return [];
    }
    // listOfLi flags: emit list-items edit to preserve <li> structure.
    if (current.xmlShape === "listOfLi") {
      return [{
        type: "setNestedListItems",
        parentNodeId: itemNodeId,
        objectPath,
        fieldName: xmlName,
        items: curAll,
        ...(parentFieldOrder?.length ? { fieldOrder: parentFieldOrder } : {}),
      }];
    }
    // flagsText (and any other shape): emit text edit.
    const text = curAll.join(", ");
    if (objectPath.length === 0) {
      return text
        ? [{ type: "setObjectListItemChildText", listItemNodeId: itemNodeId, childName: xmlName, value: text, ...(parentFieldOrder?.length ? { fieldOrder: parentFieldOrder } : {}) }]
        : [{ type: "removeObjectListItemChild", listItemNodeId: itemNodeId, childName: xmlName }];
    }
    return text
      ? [{ type: "setNestedElementText", parentNodeId: itemNodeId, objectPath, fieldName: xmlName, value: text, ...(parentFieldOrder?.length ? { fieldOrder: parentFieldOrder } : {}) }]
      : [{ type: "removeNestedElement", parentNodeId: itemNodeId, objectPath, fieldName: xmlName, pruneEmptyAncestors: true }];
  }

  // Named map - emit per-entry edits (add/remove/change).
  if (current.kind === "namedMap") {
    const initEntries = initial?.kind === "namedMap" ? initial.entries : [];
    const edits: XmlEdit[] = [];
    const initByKey = new Map(initEntries.map((e) => [e.key, e.value]));
    const curKeys = new Set(current.entries.map((e) => e.key));
    const fo = parentFieldOrder?.length ? { fieldOrder: parentFieldOrder } : {};
    for (const e of initEntries) {
      if (!curKeys.has(e.key)) {
        edits.push({ type: "removeNamedMapEntry", parentNodeId: itemNodeId, objectPath, mapName: xmlName, key: e.key });
      }
    }
    for (const e of current.entries) {
      const prev = initByKey.get(e.key);
      if (prev === undefined || prev !== e.value) {
        edits.push({ type: "setNamedMapEntry", parentNodeId: itemNodeId, objectPath, mapName: xmlName, key: e.key, value: e.value, ...fo });
      }
    }
    return edits;
  }

  // Typed reference list
  if (current.kind === "typedReferenceList") {
    const initItems = initial?.kind === "typedReferenceList" ? initial.items : [];
    const same =
      current.items.length === initItems.length &&
      current.items.every((x, i) => x.defType === initItems[i].defType && x.defName === initItems[i].defName);
    if (same) return [];
    return [{
      type: "setTypedReferenceListItems",
      parentNodeId: itemNodeId,
      objectPath,
      fieldName: xmlName,
      items: current.items.map((i) => ({ defType: i.defType, defName: i.defName })),
    }];
  }

  // Scalar-like (scalar, boolean, enum)
  const curText = objectFieldValueToText(current);
  const initText = initial ? objectFieldValueToText(initial) : null;
  if (curText === initText) return [];

  if (objectPath.length === 0) {
    // Direct child of item
    return curText
      ? [{ type: "setObjectListItemChildText", listItemNodeId: itemNodeId, childName: xmlName, value: curText, ...(parentFieldOrder?.length ? { fieldOrder: parentFieldOrder } : {}) }]
      : [{ type: "removeObjectListItemChild", listItemNodeId: itemNodeId, childName: xmlName }];
  }
  // Nested under one or more object elements
  return curText
    ? [{ type: "setNestedElementText", parentNodeId: itemNodeId, objectPath, fieldName: xmlName, value: curText, ...(parentFieldOrder?.length ? { fieldOrder: parentFieldOrder } : {}) }]
    : [{ type: "removeNestedElement", parentNodeId: itemNodeId, objectPath, fieldName: xmlName, pruneEmptyAncestors: true }];
}

function clearFieldToXmlEdit(field: FieldEditInput): XmlEdit[] {
  const path = field.model.path;
  const parentNodeId = field.model.defNodeId;

  switch (path.kind) {
    case "childElement":
      return [{ type: "removeChildElement", parentNodeId, childName: path.childName }];
    case "listItems":
      return [{ type: "removeChildElement", parentNodeId, childName: path.childName }];
    case "attribute":
      return [{ type: "removeElementAttribute", elementNodeId: parentNodeId, attributeName: path.attributeName }];
    case "nestedAttribute":
      if (field.model.sourceNodeId !== null) {
        return [{ type: "removeElementAttribute", elementNodeId: field.model.sourceNodeId, attributeName: path.attributeName }];
      }
      return [];
    case "nestedObjectField":
      return [{ type: "removeNestedElement", parentNodeId, objectPath: path.objectPath, fieldName: path.fieldName, pruneEmptyAncestors: true }];
    case "nestedListItems":
      return [{ type: "removeNestedElement", parentNodeId, objectPath: path.objectPath, fieldName: path.fieldName, pruneEmptyAncestors: true }];
    case "namedMap":
      if (path.objectPath.length === 0) {
        return [{ type: "removeChildElement", parentNodeId, childName: path.mapName }];
      }
      return [{ type: "removeNestedElement", parentNodeId, objectPath: path.objectPath, fieldName: path.mapName, pruneEmptyAncestors: true }];
    case "objectList":
      if (path.objectPath.length === 0) {
        return [{ type: "removeChildElement", parentNodeId, childName: path.fieldName }];
      }
      return [{ type: "removeNestedElement", parentNodeId, objectPath: path.objectPath, fieldName: path.fieldName, pruneEmptyAncestors: true }];
    case "typedReferenceList":
      if (path.objectPath.length === 0) {
        return [{ type: "removeChildElement", parentNodeId, childName: path.fieldName }];
      }
      return [{ type: "removeNestedElement", parentNodeId, objectPath: path.objectPath, fieldName: path.fieldName, pruneEmptyAncestors: true }];
    default:
      return [];
  }
}

export function fieldToXmlEdit(field: FieldEditInput): XmlEdit[] {
  if (field.clearRequested) {
    return clearFieldToXmlEdit(field);
  }

  const path = field.model.path;

  if (path.kind === "typedReferenceList" && field.value.kind === "typedReferenceList") {
    return [{
      type: "setTypedReferenceListItems",
      parentNodeId: field.model.defNodeId,
      objectPath: path.objectPath,
      fieldName: path.fieldName,
      items: field.value.items.map((i) => ({ defType: i.defType, defName: i.defName })),
    }];
  }

  if (path.kind === "listItems" && field.value.kind === "list") {
    return [{
      type: "setListItems",
      parentNodeId: field.model.defNodeId,
      childName: path.childName,
      items: field.value.items,
    }];
  }

  if (path.kind === "listItems" && field.value.kind === "flags") {
    return [{
      type: "setListItems",
      parentNodeId: field.model.defNodeId,
      childName: path.childName,
      items: [...field.value.selected, ...field.value.custom],
    }];
  }

  if (path.kind === "childElement") {
    const serialized =
      field.value.kind === "flags"
        ? [...field.value.selected, ...field.value.custom].join(", ")
        : formValueToString(field.value);
    return [{
      type: "setChildElementText",
      parentNodeId: field.model.defNodeId,
      childName: path.childName,
      value: serialized,
    }];
  }

  if (path.kind === "attribute") {
    return [{
      type: "setElementAttribute",
      elementNodeId: field.model.defNodeId,
      attributeName: path.attributeName,
      value: formValueToString(field.value),
    }];
  }

  if (path.kind === "nestedAttribute") {
    const serialized = formValueToString(field.value);
    if (field.model.sourceNodeId !== null) {
      return [{
        type: "setElementAttribute",
        elementNodeId: field.model.sourceNodeId,
        attributeName: path.attributeName,
        value: serialized,
      }];
    } else {
      return [{
        type: "setNestedElementAttribute",
        parentNodeId: field.model.defNodeId,
        objectPath: path.objectPath,
        attributeName: path.attributeName,
        value: serialized,
      }];
    }
  }

  if (path.kind === "nestedObjectField") {
    const serialized =
      field.value.kind === "flags"
        ? [...field.value.selected, ...field.value.custom].join(", ")
        : formValueToString(field.value);
    return [{
      type: "setNestedElementText",
      parentNodeId: field.model.defNodeId,
      objectPath: path.objectPath,
      fieldName: path.fieldName,
      value: serialized,
    }];
  }

  if (path.kind === "nestedListItems" && field.value.kind === "list") {
    return [{
      type: "setNestedListItems",
      parentNodeId: field.model.defNodeId,
      objectPath: path.objectPath,
      fieldName: path.fieldName,
      items: field.value.items,
    }];
  }

  if (path.kind === "nestedListItems" && field.value.kind === "flags") {
    return [{
      type: "setNestedListItems",
      parentNodeId: field.model.defNodeId,
      objectPath: path.objectPath,
      fieldName: path.fieldName,
      items: [...field.value.selected, ...field.value.custom],
    }];
  }

  if (path.kind === "namedMap" && field.value.kind === "namedMap" && field.initialValue.kind === "namedMap") {
    // Repeatable fields allow duplicate keys - replace atomically instead of key-based diff.
    if (field.model.repeatable) {
      return [{
        type: "replaceKeyedValueListEntries",
        parentNodeId: field.model.defNodeId,
        objectPath: path.objectPath,
        mapName: path.mapName,
        entries: field.value.entries,
      }];
    }

    const initial = field.initialValue.entries;
    const current = field.value.entries;
    const edits: XmlEdit[] = [];

    const initialKeySet = new Set(initial.map((e) => e.key));
    const currentKeySet = new Set(current.map((e) => e.key));
    const initialByKey = new Map(initial.map((e) => [e.key, e.value]));

    // Detect renames: at the same position, the key changed from a key that no
    // longer exists to a key that did not previously exist. Emit renameNamedMapEntry
    // so the backend preserves per-entry attributes (e.g. MayRequire).
    const renamedFromKeys = new Set<string>();
    const renamedToKeys = new Set<string>();
    const limit = Math.min(initial.length, current.length);
    for (let i = 0; i < limit; i++) {
      const oldKey = initial[i].key;
      const newKey = current[i].key;
      if (oldKey !== newKey && !currentKeySet.has(oldKey) && !initialKeySet.has(newKey)) {
        renamedFromKeys.add(oldKey);
        renamedToKeys.add(newKey);
        edits.push({
          type: "renameNamedMapEntry",
          parentNodeId: field.model.defNodeId,
          objectPath: path.objectPath,
          mapName: path.mapName,
          oldKey,
          newKey,
        });
        // If value also changed at this position, update it after the rename.
        if (initial[i].value !== current[i].value) {
          edits.push({
            type: "setNamedMapEntry",
            parentNodeId: field.model.defNodeId,
            objectPath: path.objectPath,
            mapName: path.mapName,
            key: newKey,
            value: current[i].value,
          });
        }
      }
    }

    // Removed entries (excluding renamed keys)
    for (const entry of initial) {
      if (!currentKeySet.has(entry.key) && !renamedFromKeys.has(entry.key)) {
        edits.push({
          type: "removeNamedMapEntry",
          parentNodeId: field.model.defNodeId,
          objectPath: path.objectPath,
          mapName: path.mapName,
          key: entry.key,
        });
      }
    }

    // Added or modified entries (excluding rename targets, already handled)
    for (const entry of current) {
      if (renamedToKeys.has(entry.key)) continue;
      const initialValue = initialByKey.get(entry.key);
      if (initialValue === undefined || initialValue !== entry.value) {
        edits.push({
          type: "setNamedMapEntry",
          parentNodeId: field.model.defNodeId,
          objectPath: path.objectPath,
          mapName: path.mapName,
          key: entry.key,
          value: entry.value,
        });
      }
    }

    return edits;
  }

  if (
    path.kind === "objectList" &&
    field.model.xmlShape === "keyedObjectMap" &&
    field.value.kind === "objectList" &&
    field.initialValue.kind === "objectList"
  ) {
    const initial = field.initialValue.items;
    const current = field.value.items;
    const edits: XmlEdit[] = [];
    const listName = path.fieldName;
    const parentNodeId = field.model.defNodeId;

    // Removed items
    const currentNodeIds = new Set(current.filter((i) => i.nodeId !== null).map((i) => i.nodeId!));
    for (const item of initial) {
      if (item.nodeId !== null && !currentNodeIds.has(item.nodeId)) {
        edits.push({ type: "removeObjectListItem", listItemNodeId: item.nodeId, pruneEmptyAncestors: true });
      }
    }

    const initialByNodeId = new Map(initial.filter((i) => i.nodeId !== null).map((i) => [i.nodeId!, i]));

    for (const item of current) {
      if (item.nodeId === null) {
        // New entry: build <li><key>keyText</key><value>fields</value></li>
        const valueChildren = buildInitialChildren(item.fields, item.fieldXmlNames, item.fieldOrder);
        edits.push({
          type: "insertObjectListItem",
          parentNodeId,
          objectPath: path.objectPath.length > 0 ? path.objectPath : undefined,
          listName,
          initialChildren: [
            { name: "key", value: item.className },
            ...(valueChildren.length > 0 ? [{ name: "value", children: valueChildren }] : []),
          ],
        });
      } else {
        const initialItem = initialByNodeId.get(item.nodeId);
        if (initialItem) {
          // Key changed: update the <key> child text
          if (item.className !== initialItem.className && item.className) {
            edits.push({
              type: "setObjectListItemChildText",
              listItemNodeId: item.nodeId,
              childName: "key",
              value: item.className,
            });
          }
          // Value field changes: all fields live inside <value>, so objectPath=["value"]
          for (const [fieldName, fieldValue] of Object.entries(item.fields)) {
            if (fieldValue.kind === "readonly") continue;
            const actualXmlName = item.fieldXmlNames?.[fieldName] ?? fieldName;
            const initialFieldValue = initialItem.fields[fieldName];
            edits.push(
              ...diffObjectFieldValue(fieldValue, initialFieldValue, item.nodeId, ["value"], actualXmlName, item.fieldOrder),
            );
          }
        }
      }
    }

    return edits;
  }

  if (
    path.kind === "objectList" &&
    field.model.xmlShape === "keyedObjectList" &&
    field.value.kind === "objectList" &&
    field.initialValue.kind === "objectList"
  ) {
    const keyField = field.model.keyField;
    const initial = field.initialValue.items;
    const current = field.value.items;
    const edits: XmlEdit[] = [];
    const listName = path.fieldName;
    const parentNodeId = field.model.defNodeId;

    // Removed items
    const currentNodeIds = new Set(current.filter((i) => i.nodeId !== null).map((i) => i.nodeId!));
    for (const item of initial) {
      if (item.nodeId !== null && !currentNodeIds.has(item.nodeId)) {
        edits.push({
          type: "removeObjectListItem",
          listItemNodeId: item.nodeId,
          pruneEmptyAncestors: true,
        });
      }
    }

    const initialByNodeId = new Map(initial.filter((i) => i.nodeId !== null).map((i) => [i.nodeId!, i]));

    for (const item of current) {
      // Key value: comes from the key field value (which mirrors className), not className directly.
      const keyValue = keyField
        ? ((item.fields[keyField] as { kind: "scalar"; value: string } | undefined)?.value ?? item.className)
        : item.className;

      if (item.nodeId === null) {
        // New item - build initial children from all non-key fields.
        const nonKeyFields = keyField
          ? Object.fromEntries(Object.entries(item.fields).filter(([k]) => k !== keyField))
          : item.fields;
        const nonKeyOrder = keyField ? item.fieldOrder?.filter((k) => k !== keyField) : item.fieldOrder;
        const initialChildren = buildInitialChildren(nonKeyFields, item.fieldXmlNames, nonKeyOrder);
        edits.push({
          type: "insertKeyedObjectListItem",
          parentNodeId,
          objectPath: path.objectPath.length > 0 ? path.objectPath : undefined,
          listName,
          keyName: keyValue,
          initialChildren: initialChildren.length > 0 ? initialChildren : undefined,
        });
      } else {
        const initialItem = initialByNodeId.get(item.nodeId);
        if (initialItem) {
          // Check if the key field changed - emit a rename.
          const initialKeyValue = keyField
            ? ((initialItem.fields[keyField] as { kind: "scalar"; value: string } | undefined)?.value ?? initialItem.className)
            : initialItem.className;
          if (keyValue !== initialKeyValue && keyValue) {
            edits.push({
              type: "renameKeyedObjectListItem",
              itemNodeId: item.nodeId,
              newName: keyValue,
            });
          }
          // Field changes - skip the key field (it's the element name, not a child element).
          for (const [fieldName, fieldValue] of Object.entries(item.fields)) {
            if (fieldName === keyField) continue;
            if (fieldValue.kind === "readonly") continue;
            const actualXmlName = item.fieldXmlNames?.[fieldName] ?? fieldName;
            const initialFieldValue = initialItem.fields[fieldName];
            if (item.attributeFields?.includes(fieldName)) {
              const curText = objectFieldValueToText(fieldValue);
              const initText = initialFieldValue !== undefined ? objectFieldValueToText(initialFieldValue) : null;
              if (curText !== initText) {
                if (curText) {
                  edits.push({ type: "setObjectListItemAttribute", listItemNodeId: item.nodeId, attributeName: actualXmlName, value: curText });
                } else {
                  edits.push({ type: "removeElementAttribute", elementNodeId: item.nodeId, attributeName: actualXmlName });
                }
              }
            } else if (item.defaultValueFieldShorthand && fieldName === field.model.defaultValueField) {
              // Item was loaded via defaultValueField shorthand (e.g. <Corpse>0.25</Corpse>).
              // Update the item element's own scalar text rather than creating a child element.
              const curText = objectFieldValueToText(fieldValue);
              const initText = initialFieldValue !== undefined ? objectFieldValueToText(initialFieldValue) : null;
              if (curText !== initText) {
                edits.push({ type: "setKeyedObjectListItemText", itemNodeId: item.nodeId, value: curText });
              }
            } else {
              edits.push(...diffObjectFieldValue(fieldValue, initialFieldValue, item.nodeId, [], actualXmlName, item.fieldOrder));
            }
          }
        }
      }
    }

    return edits;
  }

  if (
    path.kind === "objectList" &&
    field.value.kind === "objectList" &&
    field.initialValue.kind === "objectList"
  ) {
    const initial = field.initialValue.items;
    const current = field.value.items;
    const edits: XmlEdit[] = [];

    // Items present in initial but not in current = removed
    const isNested = path.objectPath.length > 0;
    const currentNodeIds = new Set(current.filter((i) => i.nodeId !== null).map((i) => i.nodeId!));
    for (const item of initial) {
      if (item.nodeId !== null && !currentNodeIds.has(item.nodeId)) {
        edits.push({
          type: "removeObjectListItem",
          listItemNodeId: item.nodeId,
          pruneEmptyAncestors: isNested || undefined,
        });
      }
    }

    // Items in current
    const initialByNodeId = new Map(initial.filter((i) => i.nodeId !== null).map((i) => [i.nodeId!, i]));
    const listName = path.fieldName;
    const parentNodeId = field.model.defNodeId;

    for (const item of current) {
      if (item.nodeId === null) {
        // New item - build full recursive initial tree.
        const initialChildren = buildInitialChildren(item.fields, item.fieldXmlNames);
        edits.push({
          type: "insertObjectListItem",
          parentNodeId,
          objectPath: path.objectPath.length > 0 ? path.objectPath : undefined,
          listName,
          classAttribute: item.className || undefined,
          initialChildren: initialChildren.length > 0 ? initialChildren : undefined,
        });
      } else {
        const initialItem = initialByNodeId.get(item.nodeId);
        // Class attribute changed
        if (initialItem && item.className !== initialItem.className) {
          edits.push({
            type: "setObjectListItemAttribute",
            listItemNodeId: item.nodeId,
            attributeName: "Class",
            value: item.className,
          });
        }
        // Field changes - recurse through ObjectFieldValue tree
        if (initialItem) {
          for (const [fieldName, fieldValue] of Object.entries(item.fields)) {
            if (fieldValue.kind === "readonly") continue;
            const actualXmlName = item.fieldXmlNames?.[fieldName] ?? fieldName;
            const initialFieldValue = initialItem.fields[fieldName];
            if (item.attributeFields?.includes(fieldName)) {
              const curText = objectFieldValueToText(fieldValue);
              const initText = initialFieldValue !== undefined ? objectFieldValueToText(initialFieldValue) : null;
              if (curText !== initText) {
                if (curText) {
                  edits.push({ type: "setObjectListItemAttribute", listItemNodeId: item.nodeId, attributeName: actualXmlName, value: curText });
                } else {
                  edits.push({ type: "removeElementAttribute", elementNodeId: item.nodeId, attributeName: actualXmlName });
                }
              }
            } else {
              edits.push(...diffObjectFieldValue(fieldValue, initialFieldValue, item.nodeId, [], actualXmlName, item.fieldOrder));
            }
          }
        }
      }
    }

    return edits;
  }

  return [];
}

/**
 * Classifies whether an edit changes document node structure (inserts/removes/reorders
 * elements). Value-only edits keep parse-order node ids stable across a re-parse, which
 * lets the controller skip a full form rebuild after a form-originated commit (Step 4).
 *
 * Note: text/attribute setters can still *create* a missing element (structural). The
 * controller additionally guards on node-count equality, so this only needs to flag the
 * edit types that can insert/remove/reorder existing elements.
 */
export function isStructuralEdit(edit: XmlEdit): boolean {
  switch (edit.type) {
    case "setChildElementText":
    case "setElementAttribute":
    case "setNestedElementText":
    case "setObjectListItemChildText":
    case "setObjectListItemAttribute":
    case "renameKeyedObjectListItem":
      return false;
    default:
      return true;
  }
}
