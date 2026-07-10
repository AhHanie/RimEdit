import type { XmlListItemView, XmlNestedChildView } from "../types/xmlDocument";
import type { FieldSchema, ObjectTypeSchema, SchemaCatalog } from "../../schema-catalog";
import type { FormControlKind, ObjectFieldValue, ObjectListItemValue } from "../types/editorForm";

type FieldContainerSchema = { fieldOrder: string[]; fields: Record<string, FieldSchema> };

const MAX_OBJECT_DEPTH = 6;

export function getOrderedSchemaFields(schema: FieldContainerSchema): [string, FieldSchema][] {
  const orderedNames = [
    ...schema.fieldOrder.filter((n) => n in schema.fields),
    ...Object.keys(schema.fields).filter((n) => !schema.fieldOrder.includes(n)),
  ];
  return orderedNames.map((name) => [name, schema.fields[name]]);
}

export function fieldSchemaToControl(
  fieldName: string,
  schema: FieldSchema,
): FormControlKind {
  if (schema.xml === "typedReferenceList") return "typedReferenceList";
  if (schema.xml === "namedChildrenMap" || schema.xml === "keyedValueList") return "namedMap";
  if (schema.xml === "keyedObjectMap") {
    return schema.items?.schemaRef ? "objectList" : "readonlyUnknown";
  }
  if (schema.xml === "flagsText") {
    return schema.validationHints?.allowedValues?.length ? "flags" : "text";
  }
  if (schema.xml === "keyedObjectList") {
    // Keyed object lists with a resolved schemaRef are editable object lists.
    if (schema.items?.kind === "object") return "objectList";
    return "list";
  }
  if (schema.xml === "listOfLi") {
    if (schema.reference) return "list";
    if (schema.flags && schema.validationHints?.allowedValues?.length) return "flags";
    if (schema.items?.kind === "object") return "objectList";
    return "list";
  }
  if (!isEditableXmlShape(schema)) return "object";
  const kind = schema.type.kind;
  if (kind === "boolean") return "checkbox";
  if (kind === "integer" || kind === "float") return "number";
  if (kind === "intRange" || kind === "floatRange") return "text";
  if (kind === "enum") {
    return schema.validationHints?.allowedValues?.length ? "select" : "text";
  }
  if (kind === "color") return "color";
  if (kind === "list") return "list";
  if (kind === "object") return "object";
  if (kind === "defReference") return "reference";
  if (fieldName === "description" || schema.xml === "text") return "textarea";
  return "text";
}

export function isEditableXmlShape(schema: FieldSchema): boolean {
  return (
    schema.xml === "element" ||
    schema.xml === "attribute" ||
    schema.xml === "text" ||
    schema.xml === "flagsText" ||
    schema.xml === "typedReferenceList"
  );
}

export const SCALAR_LIKE_CONTROLS: ReadonlySet<FormControlKind> = new Set([
  "text",
  "textarea",
  "number",
  "checkbox",
  "select",
  "reference",
  "color",
]);

/** Collect all fields (own + inherited) for an object type. */
export function getAllObjectFields(
  objectTypeName: string,
  catalog: SchemaCatalog,
): Map<string, FieldSchema> {
  const fields = new Map<string, FieldSchema>();
  const visited = new Set<string>();

  function collect(typeName: string) {
    if (visited.has(typeName)) return;
    visited.add(typeName);
    const schema = catalog.objectTypes[typeName];
    if (!schema) return;
    for (const parent of schema.inherits ?? []) collect(parent);
    for (const [name, fieldSchema] of getOrderedSchemaFields(schema)) {
      if (!fields.has(name)) fields.set(name, fieldSchema);
    }
  }

  collect(objectTypeName);
  return fields;
}

/** Resolve the effective object schema for an item using the discriminator. */
export function resolveObjectSchema(
  baseSchemaRef: string,
  className: string,
  catalog: SchemaCatalog,
): { schemaRef: string | null; schema: ObjectTypeSchema | null } {
  const baseSchema = catalog.objectTypes[baseSchemaRef];
  if (!baseSchema) return { schemaRef: null, schema: null };

  const discriminator = baseSchema.discriminator;
  if (!discriminator) return { schemaRef: baseSchemaRef, schema: baseSchema };

  if (className) {
    const variantRef = discriminator.variants[className];
    if (variantRef) {
      const variantSchema = catalog.objectTypes[variantRef];
      return { schemaRef: variantRef, schema: variantSchema ?? null };
    }
    // Unknown class: return base schema so inherited fields (e.g. Class) remain visible.
    if (discriminator.allowUnknown) {
      return { schemaRef: baseSchemaRef, schema: baseSchema };
    }
    return { schemaRef: null, schema: null };
  }

  if (discriminator.allowMissing) {
    const fallback = discriminator.fallbackSchemaRef;
    if (fallback) {
      return { schemaRef: fallback, schema: catalog.objectTypes[fallback] ?? null };
    }
    return { schemaRef: baseSchemaRef, schema: baseSchema };
  }

  return { schemaRef: null, schema: null };
}

/**
 * Build a recursive ObjectFieldValue for a single field inside an object-list item
 * or nested object. `depth` tracks how many object nesting levels deep we are;
 * rendering stops at MAX_OBJECT_DEPTH to avoid infinite recursion for self-referential
 * schemas (e.g. ThinkNode.subNodes → ThinkNode).
 */
export function buildObjectFieldValue(
  child: XmlNestedChildView | undefined,
  fieldName: string,
  fieldSchema: FieldSchema,
  catalog: SchemaCatalog,
  depth: number,
): ObjectFieldValue {
  const control = fieldSchemaToControl(fieldName, fieldSchema);

  if (control === "objectList") {
    const itemSchemaRef = fieldSchema.items?.schemaRef ?? null;
    if (!itemSchemaRef) {
      const count =
        child?.liItems?.length ?? child?.liObjectItems?.length ?? (child?.children?.length ?? 0);
      return { kind: "readonly", reason: `object list: ${count} item${count !== 1 ? "s" : ""}` };
    }
    if (depth >= MAX_OBJECT_DEPTH) {
      const count =
        child?.liItems?.length ?? child?.liObjectItems?.length ?? (child?.children?.length ?? 0);
      return { kind: "readonly", reason: `object list: ${count} item${count !== 1 ? "s" : ""} (max depth)` };
    }
    if (fieldSchema.xml === "keyedObjectMap") {
      return {
        kind: "objectList",
        itemSchemaRef,
        items: (child?.liItems ?? []).map((li) =>
          buildKeyedObjectMapItemValue(li, itemSchemaRef, catalog, depth + 1),
        ),
      };
    }
    if (fieldSchema.xml === "keyedObjectList" && fieldSchema.keyField) {
      return {
        kind: "objectList",
        itemSchemaRef,
        items: (child?.children ?? []).map((c) =>
          buildKeyedObjectListItemValue(c, fieldSchema.keyField!, fieldSchema.defaultValueField, itemSchemaRef, catalog, depth + 1),
        ),
      };
    }
    return {
      kind: "objectList",
      itemSchemaRef,
      items: (child?.liItems ?? []).map((li) =>
        buildObjectListItemValue(li, itemSchemaRef, catalog, depth + 1),
      ),
    };
  }

  if (
    fieldSchema.type.kind === "object" &&
    (fieldSchema.xml === "object" || fieldSchema.xml === "element")
  ) {
    const baseRef = fieldSchema.type.schemaRef ?? null;
    if (!baseRef) return { kind: "readonly", reason: "structured object" };
    const baseSchema = catalog.objectTypes[baseRef];
    if (!baseSchema) return { kind: "readonly", reason: "structured object" };

    let resolvedRef = baseRef;
    if (baseSchema.discriminator && child?.attributes) {
      const discAttrName = baseSchema.discriminator.attribute;
      const classAttr = child.attributes.find((a) => a.name === discAttrName);
      if (classAttr?.value) {
        const variantRef = baseSchema.discriminator.variants[classAttr.value];
        if (variantRef && catalog.objectTypes[variantRef]) {
          resolvedRef = variantRef;
        }
      }
    }

    if (depth >= MAX_OBJECT_DEPTH) return { kind: "readonly", reason: "structured object (max depth)" };
    return buildObjectValueFromNestedChild(child, resolvedRef, catalog, depth + 1);
  }

  if (control === "list") {
    return { kind: "list", items: child?.listItems ?? [] };
  }

  if (control === "flags") {
    if (fieldSchema.xml === "flagsText") {
      const text = child?.textValue ?? "";
      const all = text.split(",").map((s) => s.trim()).filter(Boolean);
      const allowed = fieldSchema.validationHints?.allowedValues ?? [];
      return {
        kind: "flags",
        selected: all.filter((v) => allowed.includes(v)),
        custom: all.filter((v) => !allowed.includes(v)),
        xmlShape: "flagsText" as const,
      };
    }
    const items = child?.listItems ?? [];
    const allowed = fieldSchema.validationHints?.allowedValues ?? [];
    return {
      kind: "flags",
      selected: items.filter((v) => allowed.includes(v)),
      custom: items.filter((v) => !allowed.includes(v)),
      xmlShape: "listOfLi" as const,
    };
  }

  if (control === "namedMap") {
    return {
      kind: "namedMap",
      entries: (child?.children ?? []).map((c) => ({ key: c.name, value: c.textValue ?? "" })),
    };
  }

  if (control === "typedReferenceList") {
    return {
      kind: "typedReferenceList",
      items: (child?.children ?? []).map((c) => ({
        nodeId: c.nodeId,
        defType: c.name,
        defName: c.textValue ?? "",
      })),
    };
  }

  const text = child?.textValue ?? "";
  const typeKind = fieldSchema.type.kind;
  if (typeKind === "boolean") {
    return { kind: "boolean", value: text === "true" || text === "True" || text === "1" };
  }
  if (typeKind === "enum") return { kind: "enum", value: text };
  return { kind: "scalar", value: text };
}

/**
 * Build an ObjectFieldValue of kind "object" from a nested child element view.
 * Recurses into the child's children for each schema field.
 */
function buildObjectValueFromNestedChild(
  child: XmlNestedChildView | undefined,
  schemaRef: string,
  catalog: SchemaCatalog,
  depth: number,
): ObjectFieldValue & { kind: "object" } {
  const allFields = getAllObjectFields(schemaRef, catalog);
  const childByName = new Map((child?.children ?? []).map((c) => [c.name, c]));
  const attributeByName = new Map((child?.attributes ?? []).map((a) => [a.name, a]));
  const fields: Record<string, ObjectFieldValue> = {};
  const fieldXmlNames: Record<string, string> = {};
  const xmlAttributeFields: string[] = [];

  for (const [fieldName, fieldSchema] of allFields) {
    if (fieldSchema.xml === "attribute") {
      // Read from element attributes, not child elements.
      const attrValue = attributeByName.get(fieldName)?.value ?? "";
      const control = fieldSchemaToControl(fieldName, fieldSchema);
      if (control === "checkbox") {
        fields[fieldName] = { kind: "boolean", value: attrValue === "true" || attrValue === "True" || attrValue === "1" };
      } else if (control === "select") {
        fields[fieldName] = { kind: "enum", value: attrValue };
      } else {
        fields[fieldName] = { kind: "scalar", value: attrValue };
      }
      xmlAttributeFields.push(fieldName);
      continue;
    }

    let nestedChild = childByName.get(fieldName);
    let actualXmlName = fieldName;
    if (!nestedChild) {
      for (const alias of fieldSchema.xmlAliases ?? []) {
        nestedChild = childByName.get(alias);
        if (nestedChild) { actualXmlName = alias; break; }
      }
    }
    if (actualXmlName !== fieldName) fieldXmlNames[fieldName] = actualXmlName;

    fields[fieldName] = buildObjectFieldValue(nestedChild, fieldName, fieldSchema, catalog, depth);
  }

  const knownNames = new Set(allFields.keys());
  for (const fs of allFields.values()) {
    for (const alias of fs.xmlAliases ?? []) knownNames.add(alias);
  }
  const initialUnknownFieldCount = (child?.children ?? []).filter((c) => !knownNames.has(c.name)).length;

  return {
    kind: "object",
    schemaRef,
    fields,
    nodeId: child?.nodeId ?? null,
    initialUnknownFieldCount,
    fieldXmlNames: Object.keys(fieldXmlNames).length > 0 ? fieldXmlNames : undefined,
    fieldOrder: [...allFields.keys()],
    xmlAttributeFields: xmlAttributeFields.length > 0 ? xmlAttributeFields : undefined,
  };
}

/**
 * Build an ObjectListItemValue from an XmlNestedChildView for a keyedObjectList item.
 *
 * The item element name is the key value (e.g. a DefName) and is stored in `className`
 * for display and tracking. The `keyField` receives the element name as a scalar value;
 * all other fields are resolved from the element's child nodes.
 */
export function buildKeyedObjectListItemValue(
  item: XmlNestedChildView,
  keyField: string,
  defaultValueField: string | null | undefined,
  baseSchemaRef: string,
  catalog: SchemaCatalog,
  depth: number = 0,
): ObjectListItemValue {
  const schema = catalog.objectTypes[baseSchemaRef] ?? null;

  if (!schema) {
    return {
      nodeId: item.nodeId,
      className: item.name,
      schemaRef: null,
      fields: {},
      initialUnknownFieldCount: (item.children ?? []).length,
    };
  }

  const allFields = getAllObjectFields(baseSchemaRef, catalog);
  const childByName = new Map((item.children ?? []).map((c) => [c.name, c]));
  const attributeByName = new Map((item.attributes ?? []).map((a) => [a.name, a]));
  const fields: Record<string, ObjectFieldValue> = {};
  const fieldXmlNames: Record<string, string> = {};
  const attributeFields: string[] = [];

  // Detect shorthand: item has scalar text (defaultValueField shorthand) and no element children.
  const shorthandText = (item.textValue ?? "").trim();
  const useShorthand = !!defaultValueField && shorthandText.length > 0 && (item.children ?? []).length === 0;

  for (const [fieldName, fieldSchema] of allFields) {
    if (fieldSchema.xml === "attribute") {
      // Keyed lists have no discriminator Class; all attribute fields go into fields.
      const attrValue = attributeByName.get(fieldName)?.value ?? "";
      const control = fieldSchemaToControl(fieldName, fieldSchema);
      if (control === "checkbox") {
        fields[fieldName] = { kind: "boolean", value: attrValue === "true" || attrValue === "True" || attrValue === "1" };
      } else if (control === "select") {
        fields[fieldName] = { kind: "enum", value: attrValue };
      } else {
        fields[fieldName] = { kind: "scalar", value: attrValue };
      }
      attributeFields.push(fieldName);
      continue;
    }

    if (fieldName === keyField) {
      // Key field is populated from the element name, not from a child element.
      fields[fieldName] = { kind: "scalar", value: item.name };
      continue;
    }

    if (useShorthand && fieldName === defaultValueField) {
      // defaultValueField shorthand: scalar text of the item element maps to this field.
      fields[fieldName] = { kind: "scalar", value: shorthandText };
      continue;
    }

    let child = childByName.get(fieldName);
    let actualXmlName = fieldName;
    if (!child) {
      for (const alias of fieldSchema.xmlAliases ?? []) {
        child = childByName.get(alias);
        if (child) { actualXmlName = alias; break; }
      }
    }
    if (actualXmlName !== fieldName) fieldXmlNames[fieldName] = actualXmlName;

    fields[fieldName] = buildObjectFieldValue(child, fieldName, fieldSchema, catalog, depth);
  }

  const knownFieldNames = new Set(allFields.keys());
  for (const fieldSchema of allFields.values()) {
    for (const alias of fieldSchema.xmlAliases ?? []) knownFieldNames.add(alias);
  }
  const initialUnknownFieldCount = (item.children ?? []).filter(
    (c) => !knownFieldNames.has(c.name),
  ).length;

  return {
    nodeId: item.nodeId,
    className: item.name,
    schemaRef: baseSchemaRef,
    fields,
    initialUnknownFieldCount,
    fieldXmlNames: Object.keys(fieldXmlNames).length > 0 ? fieldXmlNames : undefined,
    attributeFields: attributeFields.length > 0 ? attributeFields : undefined,
    fieldOrder: allFields.size > 0 ? [...allFields.keys()] : undefined,
    defaultValueFieldShorthand: useShorthand || undefined,
  };
}

/** Build an ObjectListItemValue from an XmlListItemView and the base schema ref. */
export function buildObjectListItemValue(
  item: XmlListItemView,
  baseSchemaRef: string,
  catalog: SchemaCatalog,
  depth: number = 0,
): ObjectListItemValue {
  const classAttr = item.attributes.find((a) => a.name === "Class");
  const className = classAttr?.value ?? "";

  const { schemaRef, schema } = resolveObjectSchema(baseSchemaRef, className, catalog);

  const allFields: Map<string, FieldSchema> = schema
    ? getAllObjectFields(schemaRef ?? baseSchemaRef, catalog)
    : new Map();

  if (!schema) {
    for (const [name, f] of getAllObjectFields(baseSchemaRef, catalog)) {
      if (!allFields.has(name)) allFields.set(name, f);
    }
  }

  const childByName = new Map(item.children.map((c) => [c.name, c]));
  const attributeByName = new Map(item.attributes.map((a) => [a.name, a]));
  const fields: Record<string, ObjectFieldValue> = {};
  const fieldXmlNames: Record<string, string> = {};
  const attributeFields: string[] = [];

  // The discriminator attribute (always "Class" for objectList) is already in `className`.
  const discriminatorAttrName = catalog.objectTypes[baseSchemaRef]?.discriminator?.attribute ?? "Class";

  for (const [fieldName, fieldSchema] of allFields) {
    if (fieldSchema.xml === "attribute") {
      // Skip discriminator attribute - already captured in `className`.
      if (fieldName === discriminatorAttrName) continue;
      // Non-discriminator attribute: read from item element attributes.
      const attrValue = attributeByName.get(fieldName)?.value ?? "";
      const control = fieldSchemaToControl(fieldName, fieldSchema);
      if (control === "checkbox") {
        fields[fieldName] = { kind: "boolean", value: attrValue === "true" || attrValue === "True" || attrValue === "1" };
      } else if (control === "select") {
        fields[fieldName] = { kind: "enum", value: attrValue };
      } else {
        fields[fieldName] = { kind: "scalar", value: attrValue };
      }
      attributeFields.push(fieldName);
      continue;
    }

    let child = childByName.get(fieldName);
    let actualXmlName = fieldName;
    if (!child) {
      for (const alias of fieldSchema.xmlAliases ?? []) {
        child = childByName.get(alias);
        if (child) { actualXmlName = alias; break; }
      }
    }
    if (actualXmlName !== fieldName) fieldXmlNames[fieldName] = actualXmlName;

    fields[fieldName] = buildObjectFieldValue(child, fieldName, fieldSchema, catalog, depth);
  }

  const knownFieldNames = new Set(allFields.keys());
  for (const fieldSchema of allFields.values()) {
    for (const alias of fieldSchema.xmlAliases ?? []) knownFieldNames.add(alias);
  }
  const initialUnknownFieldCount = item.children.filter(
    (c) => !knownFieldNames.has(c.name),
  ).length;

  return {
    nodeId: item.nodeId,
    className,
    schemaRef,
    fields,
    initialUnknownFieldCount,
    fieldXmlNames: Object.keys(fieldXmlNames).length > 0 ? fieldXmlNames : undefined,
    attributeFields: attributeFields.length > 0 ? attributeFields : undefined,
    fieldOrder: allFields.size > 0 ? [...allFields.keys()] : undefined,
  };
}

/**
 * Build an ObjectListItemValue from an XmlListItemView for a keyedObjectMap entry.
 *
 * Each `<li>` in a keyedObjectMap has `<key>scalar</key><value>object fields</value>`.
 * `className` is populated from the `<key>` text; fields come from the `<value>` children.
 */
export function buildKeyedObjectMapItemValue(
  li: XmlListItemView,
  baseSchemaRef: string,
  catalog: SchemaCatalog,
  depth: number = 0,
): ObjectListItemValue {
  const keyChild = li.children.find((c) => c.name === "key");
  const valueChild = li.children.find((c) => c.name === "value");
  const keyText = keyChild?.textValue ?? "";

  const schema = catalog.objectTypes[baseSchemaRef] ?? null;
  if (!schema) {
    return {
      nodeId: li.nodeId,
      className: keyText,
      schemaRef: null,
      fields: {},
      initialUnknownFieldCount: (valueChild?.children ?? []).length,
    };
  }

  const allFields = getAllObjectFields(baseSchemaRef, catalog);
  const valueChildren = valueChild?.children ?? [];
  const childByName = new Map(valueChildren.map((c) => [c.name, c]));
  const fields: Record<string, ObjectFieldValue> = {};
  const fieldXmlNames: Record<string, string> = {};

  for (const [fieldName, fieldSchema] of allFields) {
    let child: typeof valueChildren[number] | undefined = childByName.get(fieldName);
    let actualXmlName = fieldName;
    if (!child) {
      for (const alias of fieldSchema.xmlAliases ?? []) {
        child = childByName.get(alias);
        if (child) {
          actualXmlName = alias;
          break;
        }
      }
    }
    if (actualXmlName !== fieldName) fieldXmlNames[fieldName] = actualXmlName;
    fields[fieldName] = buildObjectFieldValue(child, fieldName, fieldSchema, catalog, depth);
  }

  const knownFieldNames = new Set(allFields.keys());
  for (const fs of allFields.values()) {
    for (const alias of fs.xmlAliases ?? []) knownFieldNames.add(alias);
  }
  const initialUnknownFieldCount = valueChildren.filter((c) => !knownFieldNames.has(c.name)).length;

  return {
    nodeId: li.nodeId,
    className: keyText,
    schemaRef: baseSchemaRef,
    fields,
    initialUnknownFieldCount,
    fieldXmlNames: Object.keys(fieldXmlNames).length > 0 ? fieldXmlNames : undefined,
    fieldOrder: allFields.size > 0 ? [...allFields.keys()] : undefined,
  };
}
