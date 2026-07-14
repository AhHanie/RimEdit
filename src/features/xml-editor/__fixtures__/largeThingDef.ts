// Form Views (issue 10, Plan.md section 10/13): a synthetic, realistically-shaped "large
// ThingDef" fixture -- many scalar top-level roots plus nested object/list roots -- for
// count-based (not wall-clock) performance-shape regression guards. Real
// `src-tauri/schema-packs/rimworld-core/1.6/def-types/ThingDef.json` has ~196 top-level fields;
// this fixture is the same order of magnitude, not a 2-3 field toy or a pathological outlier.
//
// Shared by `formDescriptors.largeForm.test.ts` (descriptor-construction-level proofs) and
// `useXmlFormController.largeForm.test.tsx` (controller/store-level proofs) so both layers count
// against the exact same fixture shape rather than two fixtures that could quietly drift apart.
import type { FieldSchema, ObjectTypeSchema, SchemaCatalog } from "../../schema-catalog";
import type { DefEditorView, XmlChildView, XmlNestedChildView } from "../types/xmlDocument";

export const LARGE_DEF_TYPE = "ThingDef";

/** Plain scalar top-level fields (`scalar0`..`scalarN-1`). */
export const LARGE_SCALAR_COUNT = 100;
/** Object-root top-level fields (`objectRoot0`..`objectRootN-1`), each an expandable `object`
 * shape referencing its own `ObjType{i}` object type. */
export const LARGE_OBJECT_ROOT_COUNT = 15;
/** Nested scalar fields inside every object root's object type. */
export const LARGE_NESTED_FIELDS_PER_OBJECT = 6;
/** List-root top-level fields (`listRoot0`..`listRootN-1`), `listOfLi` shape. */
export const LARGE_LIST_ROOT_COUNT = 20;
/** Of `LARGE_LIST_ROOT_COUNT`, how many are *editable object lists* (schema-backed `li` items,
 * like `comps`) rather than plain string `li` lists -- object lists pay for item-schema
 * resolution the way object roots pay for nested-descriptor expansion, so a subset must cover
 * that path too. */
export const LARGE_OBJECT_LIST_ROOT_COUNT = 3;
/** Items per object-list root -- roughly matching how a real populated `comps`/`verbs`-style
 * list looks (several entries, not one). Each object-list root produces exactly ONE descriptor
 * regardless of item count (items live inside that descriptor's `value.items` array, not as
 * separate top-level descriptors -- see `buildFormDescriptors`'s `objectList` branch), so this
 * constant does not change `LARGE_FULL_DESCRIPTOR_COUNT`; it changes how much per-item schema
 * resolution work (`buildObjectListItemValue` -> `resolveObjectSchema`/`getAllObjectFields`)
 * expanding one visible object-list root actually costs, which is exactly what the
 * hiding-skips-expansion regression guard needs to be meaningful against (a 1-item list can't
 * distinguish "resolved once for the root" from "resolved once per item"). */
export const LARGE_OBJECT_LIST_ITEM_COUNT = 8;

export const LARGE_TOTAL_TOP_LEVEL_FIELD_COUNT =
  LARGE_SCALAR_COUNT + LARGE_OBJECT_ROOT_COUNT + LARGE_LIST_ROOT_COUNT;

/** Total descriptor count when every field is visible: every scalar and list root produces
 * exactly one descriptor; every object root is replaced by its
 * `LARGE_NESTED_FIELDS_PER_OBJECT` nested descriptors instead of one descriptor of its own. */
export const LARGE_FULL_DESCRIPTOR_COUNT =
  LARGE_SCALAR_COUNT +
  LARGE_LIST_ROOT_COUNT +
  LARGE_OBJECT_ROOT_COUNT * LARGE_NESTED_FIELDS_PER_OBJECT;

export function scalarFieldId(i: number): string {
  return `scalar${i}`;
}
export function objectRootFieldId(i: number): string {
  return `objectRoot${i}`;
}
export function listRootFieldId(i: number): string {
  return `listRoot${i}`;
}
export function objectTypeRef(i: number): string {
  return `LargeObjType${i}`;
}
export function nestedFieldId(i: number): string {
  return `nested${i}`;
}
export function objectListItemTypeRef(i: number): string {
  return `LargeListItemType${i}`;
}

function scalarField(): FieldSchema {
  return {
    type: { kind: "string" },
    required: false,
    repeatable: false,
    xml: "element",
    examples: [],
    flags: false,
  };
}

/** The set of top-level field ids that are "the bulk of the field surface" -- every object root
 * and every list root, none of the plain scalars. Hiding this set (via a hypothetical "minimal"
 * Form View) is the scenario the count-based regression tests apply: it hides a large,
 * structurally expensive fraction of the form while leaving every scalar visible. */
export function bulkObjectAndListRootIds(): string[] {
  const ids: string[] = [];
  for (let i = 0; i < LARGE_OBJECT_ROOT_COUNT; i++) ids.push(objectRootFieldId(i));
  for (let i = 0; i < LARGE_LIST_ROOT_COUNT; i++) ids.push(listRootFieldId(i));
  return ids;
}

/** Every canonical top-level field id in the fixture, insertion order (scalars, then object
 * roots, then list roots) -- matches `catalog.defTypes[LARGE_DEF_TYPE].fields` key order. */
export function allTopLevelFieldIds(): string[] {
  const ids: string[] = [];
  for (let i = 0; i < LARGE_SCALAR_COUNT; i++) ids.push(scalarFieldId(i));
  ids.push(...bulkObjectAndListRootIds());
  return ids;
}

/** Only the scalar top-level field ids remain visible -- the "minimal" scenario's effective
 * visible set. */
export function scalarsOnlyVisibleSet(): Set<string> {
  const ids: string[] = [];
  for (let i = 0; i < LARGE_SCALAR_COUNT; i++) ids.push(scalarFieldId(i));
  return new Set(ids);
}

export function buildLargeThingDefCatalog(): SchemaCatalog {
  const fields: Record<string, FieldSchema> = {};
  for (let i = 0; i < LARGE_SCALAR_COUNT; i++) {
    fields[scalarFieldId(i)] = scalarField();
  }
  for (let i = 0; i < LARGE_OBJECT_ROOT_COUNT; i++) {
    fields[objectRootFieldId(i)] = {
      type: { kind: "object", schemaRef: objectTypeRef(i) },
      required: false,
      repeatable: false,
      xml: "object",
      examples: [],
      flags: false,
    };
  }
  for (let i = 0; i < LARGE_LIST_ROOT_COUNT; i++) {
    const isObjectList = i < LARGE_OBJECT_LIST_ROOT_COUNT;
    fields[listRootFieldId(i)] = {
      type: { kind: "list" },
      required: false,
      repeatable: true,
      xml: "listOfLi",
      examples: [],
      flags: false,
      ...(isObjectList
        ? { items: { kind: "object", schemaRef: objectListItemTypeRef(i) } }
        : {}),
    };
  }

  const objectTypes: Record<string, ObjectTypeSchema> = {};
  for (let i = 0; i < LARGE_OBJECT_ROOT_COUNT; i++) {
    const nestedFields: Record<string, FieldSchema> = {};
    const nestedOrder: string[] = [];
    for (let n = 0; n < LARGE_NESTED_FIELDS_PER_OBJECT; n++) {
      nestedFields[nestedFieldId(n)] = scalarField();
      nestedOrder.push(nestedFieldId(n));
    }
    objectTypes[objectTypeRef(i)] = { fieldOrder: nestedOrder, fields: nestedFields };
  }
  for (let i = 0; i < LARGE_OBJECT_LIST_ROOT_COUNT; i++) {
    objectTypes[objectListItemTypeRef(i)] = {
      fieldOrder: ["itemLabel", "itemValue"],
      fields: { itemLabel: scalarField(), itemValue: scalarField() },
    };
  }

  return {
    formatVersion: 1,
    packs: [],
    objectTypes,
    defTypes: {
      [LARGE_DEF_TYPE]: {
        inherits: [],
        abstractType: false,
        fieldOrder: allTopLevelFieldIds(),
        fields,
      },
    },
  };
}

function nestedChild(name: string, textValue: string): XmlNestedChildView {
  return {
    nodeId: -1,
    name,
    textValue,
    listItems: [],
    xmlShape: "element",
    order: 0,
    line: null,
    column: null,
  };
}

let nextNodeId = 1000;

/** A live editor-view instance for `buildLargeThingDefCatalog()`'s schema: every scalar has a
 * current value, every object root has a full set of nested values, every plain list root has
 * two `li` items, and every object-list root has `LARGE_OBJECT_LIST_ITEM_COUNT` populated items
 * (each with its own field values) -- so hiding/showing roots always has real, nontrivially-sized
 * data to preserve/restore, not merely absent optional fields or a single degenerate item. */
export function buildLargeThingDefEditorView(): DefEditorView {
  const children: XmlChildView[] = [];

  for (let i = 0; i < LARGE_SCALAR_COUNT; i++) {
    children.push({
      nodeId: nextNodeId++,
      name: scalarFieldId(i),
      textValue: `value-${i}`,
      listItems: [],
      xmlShape: "element",
      order: children.length,
      known: false,
      line: null,
      column: null,
    });
  }

  for (let i = 0; i < LARGE_OBJECT_ROOT_COUNT; i++) {
    const nested: XmlNestedChildView[] = [];
    for (let n = 0; n < LARGE_NESTED_FIELDS_PER_OBJECT; n++) {
      nested.push(nestedChild(nestedFieldId(n), `obj${i}-nested${n}`));
    }
    children.push({
      nodeId: nextNodeId++,
      name: objectRootFieldId(i),
      textValue: null,
      listItems: [],
      xmlShape: "object",
      children: nested,
      order: children.length,
      known: false,
      line: null,
      column: null,
    });
  }

  for (let i = 0; i < LARGE_LIST_ROOT_COUNT; i++) {
    const isObjectList = i < LARGE_OBJECT_LIST_ROOT_COUNT;
    if (isObjectList) {
      const liItems = [];
      for (let itemIndex = 0; itemIndex < LARGE_OBJECT_LIST_ITEM_COUNT; itemIndex++) {
        liItems.push({
          nodeId: nextNodeId++,
          textValue: null,
          attributes: [],
          children: [
            nestedChild("itemLabel", `list${i}-item${itemIndex}-label`),
            nestedChild("itemValue", `list${i}-item${itemIndex}-value`),
          ],
          order: itemIndex,
          line: null,
          column: null,
          selfClosing: false,
        });
      }
      children.push({
        nodeId: nextNodeId++,
        name: listRootFieldId(i),
        textValue: null,
        listItems: [],
        xmlShape: "listOfLi",
        order: children.length,
        known: false,
        line: null,
        column: null,
        liItems,
      });
    } else {
      children.push({
        nodeId: nextNodeId++,
        name: listRootFieldId(i),
        textValue: null,
        listItems: [`list${i}-a`, `list${i}-b`],
        xmlShape: "listOfLi",
        order: children.length,
        known: false,
        line: null,
        column: null,
      });
    }
  }

  return {
    nodeId: 0,
    defType: LARGE_DEF_TYPE,
    defName: "LargeTestDef",
    label: null,
    parentName: null,
    line: null,
    column: null,
    attributes: [],
    children,
  };
}

/** Registers a throwing getter for `catalog.objectTypes[ref]` -- reading it (discriminator
 * lookup, nested-descriptor expansion, or object-list item construction) throws immediately.
 * Used at fixture scale to prove hidden object/object-list roots' schemas are never touched,
 * not merely that hidden descriptors are absent from the final array (issue 05's
 * "hidden roots skip expensive expansion, not just post-hoc filtering" contract, exercised here
 * across many roots at once instead of one). */
export function poisonObjectType(catalog: SchemaCatalog, ref: string): void {
  Object.defineProperty(catalog.objectTypes, ref, {
    enumerable: true,
    configurable: true,
    get(): never {
      throw new Error(`hidden object type "${ref}" was resolved even though its root is hidden`);
    },
  });
}

/** Registers a non-throwing counting getter for `catalog.objectTypes[ref]` and returns a
 * function that reads the current read count. Used to prove expansion cost for a VISIBLE
 * object-list root scales with its item count (each of `LARGE_OBJECT_LIST_ITEM_COUNT` items
 * independently resolves the item schema via `buildObjectListItemValue` ->
 * `resolveObjectSchema`/`getAllObjectFields`) -- a proportional, not merely binary, regression
 * guard that a 1-item fixture could never demonstrate. */
export function countObjectTypeReads(catalog: SchemaCatalog, ref: string): () => number {
  const original = catalog.objectTypes[ref];
  let count = 0;
  Object.defineProperty(catalog.objectTypes, ref, {
    enumerable: true,
    configurable: true,
    get() {
      count += 1;
      return original;
    },
  });
  return () => count;
}
