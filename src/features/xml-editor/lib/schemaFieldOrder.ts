import type { DefTypeSchema, FieldSchema, ObjectTypeSchema, SchemaCatalog, XmlFieldShape } from "../../schema-catalog";

const ELEMENT_CHILD_SHAPES: XmlFieldShape[] = ["element", "listOfLi", "namedChildrenMap", "keyedValueList", "keyedObjectList", "object", "flagsText"];

/**
 * Compute the ordered list of element-child field names for a def type,
 * walking the full inheritance chain (ancestors first).
 *
 * Only fields with xml shape "element", "listOfLi", "namedChildrenMap", or "object"
 * are included - these are the shapes that correspond to direct child elements under
 * the Def node and are therefore affected by insertion ordering.
 *
 * "attribute" and "text" fields are excluded because they are not sibling child elements.
 */
export function buildEffectiveFieldOrder(
  defTypeName: string,
  catalog: SchemaCatalog,
): string[] {
  const chain = collectAncestorChain(defTypeName, catalog);
  const seen = new Set<string>();
  const ordered: string[] = [];

  for (const schema of chain) {
    for (const name of schema.fieldOrder) {
      if (!seen.has(name)) {
        seen.add(name);
        ordered.push(name);
      }
    }
    for (const name of Object.keys(schema.fields)) {
      if (!seen.has(name)) {
        seen.add(name);
        ordered.push(name);
      }
    }
  }

  return ordered.filter((name) => {
    for (const schema of chain) {
      if (name in schema.fields) {
        return ELEMENT_CHILD_SHAPES.includes(schema.fields[name].xml);
      }
    }
    return false;
  });
}

/**
 * Build a map of object-path → ordered element-child field names for all
 * schema-backed object fields reachable from `defTypeName`.
 *
 * Keys use dot-separated object paths (e.g. `"graphicData"`,
 * `"graphicData.shadowData"`). The value for each key is the ordered list of
 * element-child field names for children that should be inserted under that
 * object element.
 *
 * Used to send `nestedFieldOrders` in `XmlEditContext` so the backend can
 * insert newly created child elements in schema order.
 */
export function buildNestedFieldOrders(
  defTypeName: string,
  catalog: SchemaCatalog,
): Record<string, string[]> {
  const result: Record<string, string[]> = {};

  const chain = collectAncestorChain(defTypeName, catalog);
  const seen = new Set<string>();

  for (const schema of chain) {
    const orderedNames = [
      ...schema.fieldOrder.filter((n) => n in schema.fields),
      ...Object.keys(schema.fields).filter((n) => !schema.fieldOrder.includes(n)),
    ];
    for (const fieldName of orderedNames) {
      if (seen.has(fieldName)) continue;
      seen.add(fieldName);
      const fieldSchema = schema.fields[fieldName];
      if (
        fieldSchema.type.kind === "object" &&
        fieldSchema.type.schemaRef &&
        ELEMENT_CHILD_SHAPES.includes(fieldSchema.xml)
      ) {
        recurseObjectType(
          catalog,
          fieldSchema.type.schemaRef,
          fieldName,
          result,
          new Set<string>(),
        );
      }
    }
  }

  return result;
}

function recurseObjectType(
  catalog: SchemaCatalog,
  schemaRef: string,
  objectPath: string,
  result: Record<string, string[]>,
  ancestorRefs: Set<string>,
): void {
  if (ancestorRefs.has(schemaRef)) return;

  const objectSchema: ObjectTypeSchema | undefined = catalog.objectTypes[schemaRef];
  if (!objectSchema) return;

  const orderedNames = [
    ...objectSchema.fieldOrder.filter((n) => n in objectSchema.fields),
    ...Object.keys(objectSchema.fields).filter((n) => !objectSchema.fieldOrder.includes(n)),
  ];

  result[objectPath] = orderedNames.filter((name) => {
    const f: FieldSchema | undefined = objectSchema.fields[name];
    return f !== undefined && ELEMENT_CHILD_SHAPES.includes(f.xml);
  });

  const childAncestors = new Set(ancestorRefs);
  childAncestors.add(schemaRef);

  for (const fieldName of orderedNames) {
    const fieldSchema: FieldSchema | undefined = objectSchema.fields[fieldName];
    if (!fieldSchema) continue;
    if (
      fieldSchema.type.kind === "object" &&
      fieldSchema.type.schemaRef &&
      ELEMENT_CHILD_SHAPES.includes(fieldSchema.xml)
    ) {
      recurseObjectType(
        catalog,
        fieldSchema.type.schemaRef,
        `${objectPath}.${fieldName}`,
        result,
        childAncestors,
      );
    }
  }
}

function collectAncestorChain(
  typeName: string,
  catalog: SchemaCatalog,
  visited = new Set<string>(),
): DefTypeSchema[] {
  if (visited.has(typeName)) return [];
  visited.add(typeName);

  const schema = catalog.defTypes[typeName];
  if (!schema) return [];

  const result: DefTypeSchema[] = [];
  for (const parent of schema.inherits) {
    result.push(...collectAncestorChain(parent, catalog, visited));
  }
  result.push(schema);
  return result;
}
