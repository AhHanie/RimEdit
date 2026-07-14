// Form Views (issue 10, Plan.md section 10/13): count-based, non-timing performance-shape
// regression guards for descriptor construction against a realistically large (135 top-level
// field, exactly 210-descriptor -- see `LARGE_FULL_DESCRIPTOR_COUNT`) synthetic ThingDef-shaped
// fixture -- the same order of magnitude as the real `rimworld-core` `ThingDef.json` (~196
// top-level fields). Each of the fixture's 3 object-list roots carries
// `LARGE_OBJECT_LIST_ITEM_COUNT` (8) populated items, roughly matching a real populated
// `comps`/`verbs`-style list, so per-item expansion cost is actually exercised (a 1-item list
// cannot distinguish "resolved once for the root" from "resolved once per item"). Existing
// `formDescriptors.test.ts` "hidden roots skip expensive expansion" tests already prove the
// mechanism correct at 1-2 field scale; these tests prove the same contract holds across many
// hidden roots (and many-item object lists) at once, with concrete before/after counts as the
// regression guard (Plan.md section 10/13 explicitly asks for counts, not wall-clock timing).
import { buildFormDescriptors } from "./formDescriptors";
import {
  buildLargeThingDefCatalog,
  buildLargeThingDefEditorView,
  bulkObjectAndListRootIds,
  countObjectTypeReads,
  LARGE_FULL_DESCRIPTOR_COUNT,
  LARGE_OBJECT_LIST_ITEM_COUNT,
  LARGE_OBJECT_LIST_ROOT_COUNT,
  LARGE_OBJECT_ROOT_COUNT,
  LARGE_SCALAR_COUNT,
  LARGE_TOTAL_TOP_LEVEL_FIELD_COUNT,
  listRootFieldId,
  objectListItemTypeRef,
  objectRootFieldId,
  objectTypeRef,
  poisonObjectType,
  scalarsOnlyVisibleSet,
} from "../__fixtures__/largeThingDef";

describe("buildFormDescriptors - large synthetic ThingDef-shaped form (issue 10)", () => {
  it("the fixture itself is realistically large, not a toy", () => {
    // Sanity-checks the fixture's own shape so a future edit to the constants can't silently
    // shrink it back down to something that no longer exercises anything at scale.
    expect(LARGE_TOTAL_TOP_LEVEL_FIELD_COUNT).toBeGreaterThanOrEqual(100);
    expect(LARGE_OBJECT_ROOT_COUNT).toBeGreaterThanOrEqual(10);
    // A 1-item object list can't distinguish "resolved once for the root" from "resolved once
    // per item" -- each object-list root needs several items for the expansion-cost assertions
    // below to be meaningful, not trivially true.
    expect(LARGE_OBJECT_LIST_ITEM_COUNT).toBeGreaterThanOrEqual(5);
  });

  it("building the full (unfiltered) form produces exactly the expected descriptor count (210: 100 scalars + 20 list roots + 15 object roots x 6 nested fields)", () => {
    const catalog = buildLargeThingDefCatalog();
    const schema = catalog.defTypes[buildLargeThingDefEditorView().defType];
    const def = buildLargeThingDefEditorView();

    const descriptors = buildFormDescriptors(def, schema, catalog);

    expect(LARGE_FULL_DESCRIPTOR_COUNT).toBe(210);
    expect(descriptors.length).toBe(LARGE_FULL_DESCRIPTOR_COUNT);
  });

  it("hiding every object/list root (the bulk of the field surface) leaves only the scalar descriptor count, and never resolves a single hidden root's object schema", () => {
    const catalog = buildLargeThingDefCatalog();
    const schema = catalog.defTypes[buildLargeThingDefEditorView().defType];
    const def = buildLargeThingDefEditorView();

    // Poison every object type this fixture defines - every object root's own type and every
    // object-list root's item type. If ANY of the hidden roots' schema resolution is reached
    // (discriminator lookup, buildNestedObjectDescriptors, or object-list item construction),
    // this throws immediately - proving the filter short-circuits before expansion for every
    // one of them, not just a single field as the small-scale existing test does.
    for (let i = 0; i < LARGE_OBJECT_ROOT_COUNT; i++) {
      poisonObjectType(catalog, objectTypeRef(i));
    }
    for (let i = 0; i < LARGE_OBJECT_LIST_ROOT_COUNT; i++) {
      poisonObjectType(catalog, objectListItemTypeRef(i));
    }

    const visible = scalarsOnlyVisibleSet();
    let descriptors: ReturnType<typeof buildFormDescriptors> = [];
    expect(() => {
      descriptors = buildFormDescriptors(def, schema, catalog, visible);
    }).not.toThrow();

    // Concrete regression-guard counts: before (full) vs. after (scalars-only).
    expect(descriptors.length).toBe(LARGE_SCALAR_COUNT);
    expect(descriptors.length).toBeLessThan(LARGE_FULL_DESCRIPTOR_COUNT);
    for (const id of bulkObjectAndListRootIds()) {
      expect(descriptors.some((d) => d.fieldPath[0] === id)).toBe(false);
    }
  });

  it("hiding most but not all object/list roots only expands the ones still visible", () => {
    const catalog = buildLargeThingDefCatalog();
    const schema = catalog.defTypes[buildLargeThingDefEditorView().defType];
    const def = buildLargeThingDefEditorView();

    // Keep exactly one object root and one plain list root visible; hide everything else
    // (including every OTHER object/object-list root, all poisoned). A construct-then-filter
    // implementation would still touch every poisoned type while building descriptors it
    // discards afterward; this proves that never happens even in a mixed visible/hidden set.
    const keepObjectRoot = objectRootFieldId(0);
    const keepListRoot = listRootFieldId(LARGE_OBJECT_LIST_ROOT_COUNT); // a plain (non-object) list root
    for (let i = 1; i < LARGE_OBJECT_ROOT_COUNT; i++) {
      poisonObjectType(catalog, objectTypeRef(i));
    }
    for (let i = 0; i < LARGE_OBJECT_LIST_ROOT_COUNT; i++) {
      poisonObjectType(catalog, objectListItemTypeRef(i));
    }

    const visible = new Set([...scalarsOnlyVisibleSet(), keepObjectRoot, keepListRoot]);
    let descriptors: ReturnType<typeof buildFormDescriptors> = [];
    expect(() => {
      descriptors = buildFormDescriptors(def, schema, catalog, visible);
    }).not.toThrow();

    // Scalars + the one visible list root's own descriptor + the one visible object root's
    // nested descriptors (its own field id never appears as a descriptor key - it's replaced by
    // its nested fields).
    const nestedFromVisibleObjectRoot = descriptors.filter(
      (d) => d.fieldPath[0] === keepObjectRoot,
    );
    expect(nestedFromVisibleObjectRoot.length).toBeGreaterThan(0);
    expect(descriptors.some((d) => d.key === keepListRoot)).toBe(true);
    expect(descriptors.length).toBe(
      LARGE_SCALAR_COUNT + 1 /* visible list root */ + nestedFromVisibleObjectRoot.length,
    );
  });

  it("expanding a VISIBLE multi-item object-list root resolves the item schema proportionally to its item count, and hiding it resolves it zero times", () => {
    // Strengthens the throw/no-throw poisoned-getter proof above into a genuine proportional
    // measurement: with `LARGE_OBJECT_LIST_ITEM_COUNT` (8) real items, a construct-then-filter
    // regression (or one that only expands the first item) would show up as a call count that's
    // nonzero but NOT scaling with item count -- something a 1-item fixture could never surface.
    const catalog = buildLargeThingDefCatalog();
    const schema = catalog.defTypes[buildLargeThingDefEditorView().defType];
    const def = buildLargeThingDefEditorView();
    const targetRoot = listRootFieldId(0); // an object-list root (index < LARGE_OBJECT_LIST_ROOT_COUNT)
    const getReadCount = countObjectTypeReads(catalog, objectListItemTypeRef(0));

    // Visible: every item's schema is independently resolved (`resolveObjectSchema` +
    // `getAllObjectFields` each read `objectTypes[ref]` at least once per item), so the read
    // count is at least one full pass per item -- not a constant, one-time cost.
    const visibleDescriptors = buildFormDescriptors(def, schema, catalog, new Set([targetRoot]));
    expect(visibleDescriptors.some((d) => d.key === targetRoot)).toBe(true);
    expect(getReadCount()).toBeGreaterThanOrEqual(LARGE_OBJECT_LIST_ITEM_COUNT);

    // Hidden: zero reads, regardless of the 8-item list backing it -- the filter short-circuits
    // before any item is touched, not merely before the LAST one. Uses a fresh catalog/getter
    // (rather than reusing the one above) so this assertion is a clean "zero from zero", not
    // "unchanged from whatever the visible pass already accumulated".
    const freshCatalog = buildLargeThingDefCatalog();
    const freshGetReadCount = countObjectTypeReads(freshCatalog, objectListItemTypeRef(0));
    const hiddenDescriptors = buildFormDescriptors(def, schema, freshCatalog, new Set(["scalar0"]));
    expect(hiddenDescriptors.some((d) => d.key === targetRoot)).toBe(false);
    expect(freshGetReadCount()).toBe(0);
  });
});
