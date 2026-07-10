import { describe, it, expect } from "vitest";
import {
  fieldToXmlEdit,
  emptyFormValueForModel,
  objectFieldValuesEqual,
  cloneObjectFieldValue,
  emptyValueForSchema,
} from "./formValues";
import type { FormFieldModel, FormFieldPath, FormValue, ObjectFieldValue, ObjectListItemValue } from "../types/editorForm";
import type { FieldSchema } from "../../schema-catalog";

function makeModel(path: FormFieldPath, control: FormFieldModel["control"] = "text"): FormFieldModel {
  return {
    id: "test:field",
    key: "field",
    label: "Field",
    control,
    path,
    fieldPath: ["field"],
    defNodeId: 1,
    sourceNodeId: 2,
    order: 0,
    readonly: false,
    required: false,
    repeatable: false,
    xmlShape: "element",
    examples: [],
    diagnostics: [],
    sectionDefaults: [],
  };
}

function makeInput(path: FormFieldPath, value: FormValue, control: FormFieldModel["control"] = "text", clear = false) {
  const model = makeModel(path, control);
  return { model, value, initialValue: value, clearRequested: clear };
}

describe("fieldToXmlEdit – clear path", () => {
  it("childElement clear emits removeChildElement", () => {
    const edits = fieldToXmlEdit(
      makeInput(
        { kind: "childElement", childName: "description" },
        { kind: "scalar", value: "" },
        "text",
        true,
      ),
    );
    expect(edits).toEqual([{ type: "removeChildElement", parentNodeId: 1, childName: "description" }]);
  });

  it("listItems clear emits removeChildElement", () => {
    const edits = fieldToXmlEdit(
      makeInput(
        { kind: "listItems", childName: "ingredients" },
        { kind: "list", items: [] },
        "list",
        true,
      ),
    );
    expect(edits).toEqual([{ type: "removeChildElement", parentNodeId: 1, childName: "ingredients" }]);
  });

  it("attribute clear emits removeElementAttribute", () => {
    const edits = fieldToXmlEdit(
      makeInput(
        { kind: "attribute", attributeName: "Abstract" },
        { kind: "scalar", value: "" },
        "text",
        true,
      ),
    );
    expect(edits).toEqual([{ type: "removeElementAttribute", elementNodeId: 1, attributeName: "Abstract" }]);
  });

  it("nestedObjectField clear emits removeNestedElement with pruneEmptyAncestors", () => {
    const edits = fieldToXmlEdit(
      makeInput(
        { kind: "nestedObjectField", objectPath: ["graphicData"], fieldName: "texPath" },
        { kind: "scalar", value: "" },
        "text",
        true,
      ),
    );
    expect(edits).toEqual([
      { type: "removeNestedElement", parentNodeId: 1, objectPath: ["graphicData"], fieldName: "texPath", pruneEmptyAncestors: true },
    ]);
  });

  it("nestedListItems clear emits removeNestedElement with pruneEmptyAncestors", () => {
    const edits = fieldToXmlEdit(
      makeInput(
        { kind: "nestedListItems", objectPath: ["graphicData"], fieldName: "linkFlags" },
        { kind: "list", items: [] },
        "list",
        true,
      ),
    );
    expect(edits).toEqual([
      { type: "removeNestedElement", parentNodeId: 1, objectPath: ["graphicData"], fieldName: "linkFlags", pruneEmptyAncestors: true },
    ]);
  });

  it("top-level namedMap clear emits removeChildElement", () => {
    const edits = fieldToXmlEdit(
      makeInput(
        { kind: "namedMap", objectPath: [], mapName: "statBases" },
        { kind: "namedMap", entries: [] },
        "namedMap",
        true,
      ),
    );
    expect(edits).toEqual([{ type: "removeChildElement", parentNodeId: 1, childName: "statBases" }]);
  });

  it("nested namedMap clear emits removeNestedElement with pruneEmptyAncestors", () => {
    const edits = fieldToXmlEdit(
      makeInput(
        { kind: "namedMap", objectPath: ["recipeMaker"], mapName: "skillRequirements" },
        { kind: "namedMap", entries: [] },
        "namedMap",
        true,
      ),
    );
    expect(edits).toEqual([
      {
        type: "removeNestedElement",
        parentNodeId: 1,
        objectPath: ["recipeMaker"],
        fieldName: "skillRequirements",
        pruneEmptyAncestors: true,
      },
    ]);
  });

  it("top-level objectList clear emits removeChildElement", () => {
    const edits = fieldToXmlEdit(
      makeInput(
        { kind: "objectList", objectPath: [], fieldName: "comps" },
        { kind: "objectList", items: [] },
        "objectList",
        true,
      ),
    );
    expect(edits).toEqual([{ type: "removeChildElement", parentNodeId: 1, childName: "comps" }]);
  });

  it("nested objectList clear emits removeNestedElement with pruneEmptyAncestors", () => {
    const edits = fieldToXmlEdit(
      makeInput(
        { kind: "objectList", objectPath: ["recipeMaker"], fieldName: "ingredients" },
        { kind: "objectList", items: [] },
        "objectList",
        true,
      ),
    );
    expect(edits).toEqual([
      { type: "removeNestedElement", parentNodeId: 1, objectPath: ["recipeMaker"], fieldName: "ingredients", pruneEmptyAncestors: true },
    ]);
  });

  it("top-level typedReferenceList clear emits removeChildElement", () => {
    const edits = fieldToXmlEdit(
      makeInput(
        { kind: "typedReferenceList", objectPath: [], fieldName: "descriptionHyperlinks" },
        { kind: "typedReferenceList", items: [] },
        "typedReferenceList",
        true,
      ),
    );
    expect(edits).toEqual([{ type: "removeChildElement", parentNodeId: 1, childName: "descriptionHyperlinks" }]);
  });

  it("nested typedReferenceList clear emits removeNestedElement with pruneEmptyAncestors", () => {
    const edits = fieldToXmlEdit(
      makeInput(
        { kind: "typedReferenceList", objectPath: ["building"], fieldName: "blueprintGraphicData" },
        { kind: "typedReferenceList", items: [] },
        "typedReferenceList",
        true,
      ),
    );
    expect(edits).toEqual([
      { type: "removeNestedElement", parentNodeId: 1, objectPath: ["building"], fieldName: "blueprintGraphicData", pruneEmptyAncestors: true },
    ]);
  });

  it("clearRequested=false falls through to normal edit", () => {
    const edits = fieldToXmlEdit(
      makeInput(
        { kind: "childElement", childName: "description" },
        { kind: "scalar", value: "" },
        "text",
        false,
      ),
    );
    expect(edits).toEqual([
      { type: "setChildElementText", parentNodeId: 1, childName: "description", value: "" },
    ]);
  });
});

describe("emptyFormValueForModel", () => {
  it("text → scalar empty string", () => {
    const model = makeModel({ kind: "childElement", childName: "x" }, "text");
    expect(emptyFormValueForModel(model)).toEqual({ kind: "scalar", value: "" });
  });

  it("checkbox → boolean false", () => {
    const model = makeModel({ kind: "childElement", childName: "x" }, "checkbox");
    expect(emptyFormValueForModel(model)).toEqual({ kind: "boolean", value: false });
  });

  it("select → enum empty string", () => {
    const model = makeModel({ kind: "childElement", childName: "x" }, "select");
    expect(emptyFormValueForModel(model)).toEqual({ kind: "enum", value: "" });
  });

  it("list → empty list", () => {
    const model = makeModel({ kind: "listItems", childName: "x" }, "list");
    expect(emptyFormValueForModel(model)).toEqual({ kind: "list", items: [] });
  });

  it("flags → empty flags", () => {
    const model = makeModel({ kind: "listItems", childName: "x" }, "flags");
    expect(emptyFormValueForModel(model)).toEqual({ kind: "flags", selected: [], custom: [] });
  });

  it("namedMap → empty map", () => {
    const model = makeModel({ kind: "namedMap", objectPath: [], mapName: "x" }, "namedMap");
    expect(emptyFormValueForModel(model)).toEqual({ kind: "namedMap", entries: [] });
  });

  it("typedReferenceList → empty list", () => {
    const model = makeModel({ kind: "typedReferenceList", objectPath: [], fieldName: "x" }, "typedReferenceList");
    expect(emptyFormValueForModel(model)).toEqual({ kind: "typedReferenceList", items: [] });
  });

  it("objectList → empty list", () => {
    const model = makeModel({ kind: "objectList", objectPath: [], fieldName: "x" }, "objectList");
    expect(emptyFormValueForModel(model)).toEqual({ kind: "objectList", items: [] });
  });
});

// --- Issue 1: objectList alias field-name preservation ---

describe("fieldToXmlEdit – objectList alias field-name preservation", () => {
  function makeObjectListInput(
    currentItems: import("../types/editorForm").ObjectListItemValue[],
    initialItems: import("../types/editorForm").ObjectListItemValue[],
  ) {
    return {
      model: makeModel({ kind: "objectList", objectPath: [], fieldName: "subSounds" }, "objectList"),
      value: { kind: "objectList" as const, items: currentItems },
      initialValue: { kind: "objectList" as const, items: initialItems },
    };
  }

  it("uses fieldXmlNames alias when emitting setObjectListItemChildText", () => {
    const item = {
      nodeId: 10,
      className: "",
      schemaRef: "SubSoundDef",
      fields: {
        sustainIntervalRange: { kind: "scalar" as const, value: "1~2" },
      },
      initialUnknownFieldCount: 0,
      fieldXmlNames: { sustainIntervalRange: "sustainInterval" },
    };
    const edits = fieldToXmlEdit(makeObjectListInput([item], [{ ...item, fields: { sustainIntervalRange: { kind: "scalar", value: "0.5~1" } } }]));
    const textEdit = edits.find((e) => e.type === "setObjectListItemChildText") as { childName: string } | undefined;
    expect(textEdit).toBeDefined();
    expect(textEdit!.childName).toBe("sustainInterval");
  });

  it("uses fieldXmlNames alias when emitting removeObjectListItemChild", () => {
    const initialItem = {
      nodeId: 10,
      className: "",
      schemaRef: "SubSoundDef",
      fields: { sustainIntervalRange: { kind: "scalar" as const, value: "1~2" } },
      initialUnknownFieldCount: 0,
      fieldXmlNames: { sustainIntervalRange: "sustainInterval" },
    };
    const currentItem = {
      ...initialItem,
      fields: { sustainIntervalRange: { kind: "scalar" as const, value: "" } },
    };
    const edits = fieldToXmlEdit(makeObjectListInput([currentItem], [initialItem]));
    const removeEdit = edits.find((e) => e.type === "removeObjectListItemChild") as { childName: string } | undefined;
    expect(removeEdit).toBeDefined();
    expect(removeEdit!.childName).toBe("sustainInterval");
  });

  it("childElement + flags serializes as comma-separated string (flagsText)", () => {
    const edits = fieldToXmlEdit(
      makeInput(
        { kind: "childElement", childName: "developmentalStageFilter" },
        { kind: "flags", selected: ["Child", "Adult"], custom: [] },
        "flags",
      ),
    );
    expect(edits).toEqual([
      { type: "setChildElementText", parentNodeId: 1, childName: "developmentalStageFilter", value: "Child, Adult" },
    ]);
  });

  it("childElement + flags with custom tokens serializes all as comma-separated", () => {
    const edits = fieldToXmlEdit(
      makeInput(
        { kind: "childElement", childName: "stageFilter" },
        { kind: "flags", selected: ["Child"], custom: ["Unknown"] },
        "flags",
      ),
    );
    expect(edits).toEqual([
      { type: "setChildElementText", parentNodeId: 1, childName: "stageFilter", value: "Child, Unknown" },
    ]);
  });

  it("nestedObjectField + flags serializes as comma-separated string (flagsText)", () => {
    const edits = fieldToXmlEdit(
      makeInput(
        { kind: "nestedObjectField", objectPath: ["stage"], fieldName: "developmentalStageFilter" },
        { kind: "flags", selected: ["Newborn", "Baby"], custom: [] },
        "flags",
      ),
    );
    expect(edits).toEqual([
      { type: "setNestedElementText", parentNodeId: 1, objectPath: ["stage"], fieldName: "developmentalStageFilter", value: "Newborn, Baby" },
    ]);
  });

  it("skips readonly fields in objectList diff", () => {
    const initialItem = {
      nodeId: 10,
      className: "",
      schemaRef: "SubSoundDef",
      fields: {
        name: { kind: "scalar" as const, value: "old" },
        grains: { kind: "readonly" as const, reason: "object list: 2 items" },
      },
      initialUnknownFieldCount: 0,
    };
    const currentItem = {
      ...initialItem,
      fields: {
        name: { kind: "scalar" as const, value: "new" },
        grains: { kind: "readonly" as const, reason: "object list: 2 items" },
      },
    };
    const edits = fieldToXmlEdit(makeObjectListInput([currentItem], [initialItem]));
    // Only the name change should be emitted, not grains
    const grainsEdit = edits.find(
      (e) => e.type === "setObjectListItemChildText" && (e as { childName: string }).childName === "grains",
    );
    expect(grainsEdit).toBeUndefined();
    const nameEdit = edits.find(
      (e) => e.type === "setObjectListItemChildText" && (e as { childName: string }).childName === "name",
    );
    expect(nameEdit).toBeDefined();
  });
});

describe("fieldToXmlEdit – repeatable namedMap (keyedValueList with duplicate keys)", () => {
  function makeRepeatableMapInput(
    current: { key: string; value: string }[],
    initial: { key: string; value: string }[],
  ) {
    const model: FormFieldModel = {
      ...makeModel({ kind: "namedMap", objectPath: [], mapName: "nullifyingTraitDegrees" }, "namedMap"),
      repeatable: true,
      xmlShape: "keyedValueList",
    };
    return {
      model,
      value: { kind: "namedMap" as const, entries: current },
      initialValue: { kind: "namedMap" as const, entries: initial },
    };
  }

  it("generates replaceKeyedValueListEntries for repeatable map", () => {
    const entries = [
      { key: "DrugDesire", value: "0" },
      { key: "DrugDesire", value: "1" },
    ];
    const edits = fieldToXmlEdit(makeRepeatableMapInput(entries, []));
    expect(edits).toEqual([
      {
        type: "replaceKeyedValueListEntries",
        parentNodeId: 1,
        objectPath: [],
        mapName: "nullifyingTraitDegrees",
        entries,
      },
    ]);
  });

  it("replaces all entries atomically even when no duplicates", () => {
    const current = [{ key: "Psychopath", value: "0" }];
    const initial = [{ key: "Brawler", value: "0" }];
    const edits = fieldToXmlEdit(makeRepeatableMapInput(current, initial));
    expect(edits).toEqual([
      {
        type: "replaceKeyedValueListEntries",
        parentNodeId: 1,
        objectPath: [],
        mapName: "nullifyingTraitDegrees",
        entries: current,
      },
    ]);
  });

  it("non-repeatable namedMap still uses key-based diff", () => {
    const initial = [{ key: "Shooting", value: "4" }];
    const current = [{ key: "Shooting", value: "8" }];
    const model = makeModel({ kind: "namedMap", objectPath: [], mapName: "skillRequirements" }, "namedMap");
    const edits = fieldToXmlEdit({
      model,
      value: { kind: "namedMap", entries: current },
      initialValue: { kind: "namedMap", entries: initial },
    });
    expect(edits).toEqual([
      {
        type: "setNamedMapEntry",
        parentNodeId: 1,
        objectPath: [],
        mapName: "skillRequirements",
        key: "Shooting",
        value: "8",
      },
    ]);
  });

  it("emits renameNamedMapEntry when key changes at the same position", () => {
    const initial = [{ key: "MedicineIndustrial", value: "4" }];
    const current = [{ key: "MedicineHerbal", value: "4" }];
    const model = makeModel({ kind: "namedMap", objectPath: [], mapName: "possessions" }, "namedMap");
    const edits = fieldToXmlEdit({
      model,
      value: { kind: "namedMap", entries: current },
      initialValue: { kind: "namedMap", entries: initial },
    });
    expect(edits).toEqual([
      {
        type: "renameNamedMapEntry",
        parentNodeId: 1,
        objectPath: [],
        mapName: "possessions",
        oldKey: "MedicineIndustrial",
        newKey: "MedicineHerbal",
      },
    ]);
  });

  it("emits renameNamedMapEntry then setNamedMapEntry when both key and value change", () => {
    const initial = [{ key: "MedicineIndustrial", value: "4" }];
    const current = [{ key: "MedicineHerbal", value: "2" }];
    const model = makeModel({ kind: "namedMap", objectPath: [], mapName: "possessions" }, "namedMap");
    const edits = fieldToXmlEdit({
      model,
      value: { kind: "namedMap", entries: current },
      initialValue: { kind: "namedMap", entries: initial },
    });
    expect(edits).toEqual([
      {
        type: "renameNamedMapEntry",
        parentNodeId: 1,
        objectPath: [],
        mapName: "possessions",
        oldKey: "MedicineIndustrial",
        newKey: "MedicineHerbal",
      },
      {
        type: "setNamedMapEntry",
        parentNodeId: 1,
        objectPath: [],
        mapName: "possessions",
        key: "MedicineHerbal",
        value: "2",
      },
    ]);
  });

  it("does not emit rename when the new key already existed in the initial map", () => {
    const initial = [{ key: "A", value: "1" }, { key: "B", value: "2" }];
    const current = [{ key: "B", value: "1" }, { key: "B", value: "2" }];
    const model = makeModel({ kind: "namedMap", objectPath: [], mapName: "possessions" }, "namedMap");
    const edits = fieldToXmlEdit({
      model,
      value: { kind: "namedMap", entries: current },
      initialValue: { kind: "namedMap", entries: initial },
    });
    // A is removed; B at position 0 is treated as a value update, not a rename
    expect(edits.every((e) => e.type !== "renameNamedMapEntry")).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// objectFieldValuesEqual
// ---------------------------------------------------------------------------

describe("objectFieldValuesEqual", () => {
  it("two equal scalars are equal", () => {
    expect(objectFieldValuesEqual({ kind: "scalar", value: "x" }, { kind: "scalar", value: "x" })).toBe(true);
  });

  it("scalar vs different scalar are not equal", () => {
    expect(objectFieldValuesEqual({ kind: "scalar", value: "x" }, { kind: "scalar", value: "y" })).toBe(false);
  });

  it("two equal objectList values are equal", () => {
    const item: ObjectListItemValue = {
      nodeId: 1,
      className: "",
      schemaRef: "Foo",
      fields: { x: { kind: "scalar", value: "v" } },
      initialUnknownFieldCount: 0,
    };
    const a: ObjectFieldValue = { kind: "objectList", itemSchemaRef: "Foo", items: [item] };
    const b: ObjectFieldValue = {
      kind: "objectList",
      itemSchemaRef: "Foo",
      items: [{ ...item, fields: { x: { kind: "scalar", value: "v" } } }],
    };
    expect(objectFieldValuesEqual(a, b)).toBe(true);
  });

  it("two objectList values differing by a nested scalar are not equal", () => {
    const a: ObjectFieldValue = {
      kind: "objectList",
      itemSchemaRef: "Foo",
      items: [{ nodeId: 1, className: "", schemaRef: "Foo", fields: { x: { kind: "scalar", value: "v" } }, initialUnknownFieldCount: 0 }],
    };
    const b: ObjectFieldValue = {
      kind: "objectList",
      itemSchemaRef: "Foo",
      items: [{ nodeId: 1, className: "", schemaRef: "Foo", fields: { x: { kind: "scalar", value: "w" } }, initialUnknownFieldCount: 0 }],
    };
    expect(objectFieldValuesEqual(a, b)).toBe(false);
  });

  it("two equal object values are equal", () => {
    const a: ObjectFieldValue = {
      kind: "object",
      schemaRef: "Foo",
      fields: { x: { kind: "scalar", value: "v" } },
      nodeId: null,
      initialUnknownFieldCount: 0,
      fieldOrder: [],
    };
    const b: ObjectFieldValue = {
      kind: "object",
      schemaRef: "Foo",
      fields: { x: { kind: "scalar", value: "v" } },
      nodeId: null,
      initialUnknownFieldCount: 0,
      fieldOrder: [],
    };
    expect(objectFieldValuesEqual(a, b)).toBe(true);
  });

  it("object with extra key vs one without are not equal", () => {
    const base: ObjectFieldValue = {
      kind: "object",
      schemaRef: "Foo",
      fields: { x: { kind: "scalar", value: "v" } },
      nodeId: null,
      initialUnknownFieldCount: 0,
      fieldOrder: [],
    };
    const withExtra: ObjectFieldValue = {
      kind: "object",
      schemaRef: "Foo",
      fields: { x: { kind: "scalar", value: "v" }, y: { kind: "scalar", value: "w" } },
      nodeId: null,
      initialUnknownFieldCount: 0,
      fieldOrder: [],
    };
    expect(objectFieldValuesEqual(base, withExtra)).toBe(false);
  });

  it("readonly kinds are always equal regardless of reason text", () => {
    expect(objectFieldValuesEqual({ kind: "readonly", reason: "a" }, { kind: "readonly", reason: "b" })).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// cloneObjectFieldValue
// ---------------------------------------------------------------------------

describe("cloneObjectFieldValue", () => {
  it("cloning an objectList produces a new array (not reference-equal)", () => {
    const orig: ObjectFieldValue = {
      kind: "objectList",
      itemSchemaRef: "Foo",
      items: [{ nodeId: 1, className: "", schemaRef: "Foo", fields: { x: { kind: "scalar", value: "v" } }, initialUnknownFieldCount: 0 }],
    };
    const clone = cloneObjectFieldValue(orig);
    expect(clone.kind).toBe("objectList");
    if (clone.kind === "objectList") {
      expect(clone.items).not.toBe(orig.kind === "objectList" ? orig.items : null);
    }
  });

  it("mutating a cloned item's field does not affect the original", () => {
    const orig: ObjectFieldValue = {
      kind: "objectList",
      itemSchemaRef: "Foo",
      items: [{ nodeId: 1, className: "", schemaRef: "Foo", fields: { x: { kind: "scalar", value: "v" } }, initialUnknownFieldCount: 0 }],
    };
    const clone = cloneObjectFieldValue(orig);
    if (clone.kind === "objectList") {
      clone.items[0].fields.x = { kind: "scalar", value: "changed" };
    }
    expect(orig.kind === "objectList" && orig.items[0].fields.x).toEqual({ kind: "scalar", value: "v" });
  });

  it("cloning an object value recursively clones nested fields", () => {
    const orig: ObjectFieldValue = {
      kind: "object",
      schemaRef: "Foo",
      fields: { nested: { kind: "scalar", value: "v" } },
      nodeId: null,
      initialUnknownFieldCount: 0,
      fieldOrder: [],
    };
    const clone = cloneObjectFieldValue(orig);
    if (clone.kind === "object") {
      clone.fields.nested = { kind: "scalar", value: "changed" };
    }
    expect(orig.kind === "object" && orig.fields.nested).toEqual({ kind: "scalar", value: "v" });
  });
});

// ---------------------------------------------------------------------------
// emptyValueForSchema
// ---------------------------------------------------------------------------

describe("emptyValueForSchema", () => {
  function fs(xml: FieldSchema["xml"], typeKind: FieldSchema["type"]["kind"] = "string", extra?: Partial<FieldSchema>): FieldSchema {
    return { type: { kind: typeKind }, xml, required: false, examples: [], repeatable: false, flags: false, ...extra };
  }

  it("element string → scalar empty string", () => {
    expect(emptyValueForSchema(fs("element", "string"))).toEqual({ kind: "scalar", value: "" });
  });

  it("element boolean → boolean false", () => {
    expect(emptyValueForSchema(fs("element", "boolean"))).toEqual({ kind: "boolean", value: false });
  });

  it("listOfLi without object items → list", () => {
    expect(emptyValueForSchema(fs("listOfLi"))).toEqual({ kind: "list", items: [] });
  });

  it("listOfLi with object items → undefined", () => {
    expect(emptyValueForSchema(fs("listOfLi", "list", { items: { kind: "object" } }))).toBeUndefined();
  });

  it("flagsText → flags with flagsText xmlShape", () => {
    expect(emptyValueForSchema(fs("flagsText"))).toEqual({ kind: "flags", selected: [], custom: [], xmlShape: "flagsText" });
  });

  it("namedChildrenMap → namedMap empty", () => {
    expect(emptyValueForSchema(fs("namedChildrenMap"))).toEqual({ kind: "namedMap", entries: [] });
  });

  it("typedReferenceList → typedReferenceList empty", () => {
    expect(emptyValueForSchema(fs("typedReferenceList"))).toEqual({ kind: "typedReferenceList", items: [] });
  });

  it("object type → undefined", () => {
    expect(emptyValueForSchema(fs("element", "object"))).toBeUndefined();
  });
});

// ---------------------------------------------------------------------------
// fieldToXmlEdit – nested edits relative to item node id
// ---------------------------------------------------------------------------

describe("fieldToXmlEdit – nested edits relative to item node id", () => {
  function makeObjectListInput(
    currentItems: ObjectListItemValue[],
    initialItems: ObjectListItemValue[],
  ) {
    return {
      model: makeModel({ kind: "objectList", objectPath: [], fieldName: "subSounds" }, "objectList"),
      value: { kind: "objectList" as const, items: currentItems },
      initialValue: { kind: "objectList" as const, items: initialItems },
    };
  }

  it("emits setNestedElementText when a field nested under an object changes", () => {
    const paramsBase: ObjectFieldValue & { kind: "object" } = {
      kind: "object",
      schemaRef: "SoundParams",
      fields: { volume: { kind: "scalar", value: "0.5" } },
      nodeId: null,
      initialUnknownFieldCount: 0,
      fieldOrder: [],
    };
    const initial: ObjectListItemValue = {
      nodeId: 10,
      className: "",
      schemaRef: "SubSoundDef",
      fields: { params: paramsBase },
      initialUnknownFieldCount: 0,
    };
    const current: ObjectListItemValue = {
      ...initial,
      fields: { params: { ...paramsBase, fields: { volume: { kind: "scalar", value: "0.9" } } } },
    };
    const edits = fieldToXmlEdit(makeObjectListInput([current], [initial]));
    const nested = edits.find((e) => e.type === "setNestedElementText") as
      | { parentNodeId: number; objectPath: string[]; fieldName: string }
      | undefined;
    expect(nested).toBeDefined();
    expect(nested!.parentNodeId).toBe(10);
    expect(nested!.objectPath).toEqual(["params"]);
    expect(nested!.fieldName).toBe("volume");
  });

  it("emits setObjectListItemChildText when a direct scalar child changes", () => {
    const initial: ObjectListItemValue = {
      nodeId: 10,
      className: "",
      schemaRef: "SubSoundDef",
      fields: { name: { kind: "scalar", value: "old" } },
      initialUnknownFieldCount: 0,
    };
    const current: ObjectListItemValue = { ...initial, fields: { name: { kind: "scalar", value: "new" } } };
    const edits = fieldToXmlEdit(makeObjectListInput([current], [initial]));
    const e = edits.find((e) => e.type === "setObjectListItemChildText") as
      | { listItemNodeId: number; childName: string }
      | undefined;
    expect(e).toBeDefined();
    expect(e!.listItemNodeId).toBe(10);
    expect(e!.childName).toBe("name");
  });

  it("emits removeObjectListItemChild when a direct scalar becomes empty", () => {
    const initial: ObjectListItemValue = {
      nodeId: 10,
      className: "",
      schemaRef: "SubSoundDef",
      fields: { name: { kind: "scalar", value: "old" } },
      initialUnknownFieldCount: 0,
    };
    const current: ObjectListItemValue = { ...initial, fields: { name: { kind: "scalar", value: "" } } };
    const edits = fieldToXmlEdit(makeObjectListInput([current], [initial]));
    const e = edits.find((e) => e.type === "removeObjectListItemChild") as
      | { listItemNodeId: number; childName: string }
      | undefined;
    expect(e).toBeDefined();
    expect(e!.listItemNodeId).toBe(10);
    expect(e!.childName).toBe("name");
  });

  it("emits insertObjectListItem for a new sub-item in a nested objectList", () => {
    const initial: ObjectListItemValue = {
      nodeId: 10,
      className: "",
      schemaRef: "SubSoundDef",
      fields: {
        grains: { kind: "objectList", itemSchemaRef: "AudioGrain", items: [] },
      },
      initialUnknownFieldCount: 0,
    };
    const current: ObjectListItemValue = {
      ...initial,
      fields: {
        grains: {
          kind: "objectList",
          itemSchemaRef: "AudioGrain",
          items: [
            {
              nodeId: null,
              className: "AudioGrain_Clip",
              schemaRef: "AudioGrain_Clip",
              fields: {},
              initialUnknownFieldCount: 0,
            },
          ],
        },
      },
    };
    const edits = fieldToXmlEdit(makeObjectListInput([current], [initial]));
    const insert = edits.find((e) => e.type === "insertObjectListItem") as
      | { parentNodeId: number; listName: string }
      | undefined;
    expect(insert).toBeDefined();
    expect(insert!.parentNodeId).toBe(10);
    expect(insert!.listName).toBe("grains");
  });

  it("setObjectListItemChildText carries fieldOrder from item.fieldOrder", () => {
    const initial: ObjectListItemValue = {
      nodeId: 10,
      className: "",
      schemaRef: "SubSoundDef",
      fields: { name: { kind: "scalar", value: "old" } },
      initialUnknownFieldCount: 0,
      fieldOrder: ["name", "volume"],
    };
    const current: ObjectListItemValue = { ...initial, fields: { name: { kind: "scalar", value: "new" } } };
    const edits = fieldToXmlEdit(makeObjectListInput([current], [initial]));
    const e = edits.find((e) => e.type === "setObjectListItemChildText") as
      | { fieldOrder?: string[] }
      | undefined;
    expect(e).toBeDefined();
    expect(e!.fieldOrder).toEqual(["name", "volume"]);
  });

  it("setNestedElementText carries fieldOrder when nested under an object with a fieldOrder", () => {
    const paramsBase: ObjectFieldValue & { kind: "object" } = {
      kind: "object",
      schemaRef: "SoundParams",
      fields: { volume: { kind: "scalar", value: "0.5" } },
      nodeId: null,
      initialUnknownFieldCount: 0,
      fieldOrder: ["volume", "pitch"],
    };
    const initial: ObjectListItemValue = {
      nodeId: 10,
      className: "",
      schemaRef: "SubSoundDef",
      fields: { params: paramsBase },
      initialUnknownFieldCount: 0,
    };
    const current: ObjectListItemValue = {
      ...initial,
      fields: { params: { ...paramsBase, fields: { volume: { kind: "scalar", value: "0.9" } } } },
    };
    const edits = fieldToXmlEdit(makeObjectListInput([current], [initial]));
    const nested = edits.find((e) => e.type === "setNestedElementText") as
      | { fieldOrder?: string[] }
      | undefined;
    expect(nested).toBeDefined();
    expect(nested!.fieldOrder).toEqual(["volume", "pitch"]);
  });
});

describe("fieldToXmlEdit – nestedAttribute path", () => {
  function makeNestedAttrModel(sourceNodeId: number | null): FormFieldModel {
    return {
      id: "test:root.Class",
      key: "root.Class",
      label: "Class",
      control: "text",
      path: { kind: "nestedAttribute", objectPath: ["root"], attributeName: "Class" },
      fieldPath: ["root", "Class"],
      defNodeId: 1,
      sourceNodeId,
      order: 0,
      readonly: false,
      required: false,
      repeatable: false,
      xmlShape: "attribute",
      examples: [],
      diagnostics: [],
      sectionDefaults: [],
    };
  }

  it("emits setElementAttribute against sourceNodeId when target node exists", () => {
    const model = makeNestedAttrModel(20);
    const edits = fieldToXmlEdit({ model, value: { kind: "scalar", value: "QuestNode_Chance" }, initialValue: { kind: "scalar", value: "" } });
    expect(edits).toEqual([{
      type: "setElementAttribute",
      elementNodeId: 20,
      attributeName: "Class",
      value: "QuestNode_Chance",
    }]);
  });

  it("emits setNestedElementAttribute when object node is absent", () => {
    const model = makeNestedAttrModel(null);
    const edits = fieldToXmlEdit({ model, value: { kind: "scalar", value: "QuestNode_Sequence" }, initialValue: { kind: "scalar", value: "" } });
    expect(edits).toEqual([{
      type: "setNestedElementAttribute",
      parentNodeId: 1,
      objectPath: ["root"],
      attributeName: "Class",
      value: "QuestNode_Sequence",
    }]);
  });

  it("clear emits removeElementAttribute when target node exists", () => {
    const model = makeNestedAttrModel(20);
    const edits = fieldToXmlEdit({ model, value: { kind: "scalar", value: "" }, initialValue: { kind: "scalar", value: "QuestNode_Sequence" }, clearRequested: true });
    expect(edits).toEqual([{
      type: "removeElementAttribute",
      elementNodeId: 20,
      attributeName: "Class",
    }]);
  });

  it("clear on absent target emits no edit", () => {
    const model = makeNestedAttrModel(null);
    const edits = fieldToXmlEdit({ model, value: { kind: "scalar", value: "" }, initialValue: { kind: "scalar", value: "" }, clearRequested: true });
    expect(edits).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// keyedObjectMap edit serialization
// ---------------------------------------------------------------------------

describe("fieldToXmlEdit – keyedObjectMap path", () => {
  function makeKeyedMapModel(): FormFieldModel {
    return {
      ...makeModel({ kind: "objectList", objectPath: [], fieldName: "keyedMap" }, "objectList"),
      xmlShape: "keyedObjectMap",
    };
  }

  function makeItem(nodeId: number | null, key: string, fieldValue: string): ObjectListItemValue {
    return {
      nodeId,
      className: key,
      schemaRef: "PartValue",
      fields: { knownField: { kind: "scalar", value: fieldValue } },
      initialUnknownFieldCount: 0,
      fieldOrder: ["knownField"],
    };
  }

  function makeInput(
    currentItems: ObjectListItemValue[],
    initialItems: ObjectListItemValue[],
  ) {
    const model = makeKeyedMapModel();
    return {
      model,
      value: { kind: "objectList" as const, items: currentItems },
      initialValue: { kind: "objectList" as const, items: initialItems },
    };
  }

  it("inserts new item with <key>/<value> initial children", () => {
    const newItem = makeItem(null, "Root", "hello");
    const edits = fieldToXmlEdit(makeInput([newItem], []));
    const insert = edits.find((e) => e.type === "insertObjectListItem") as
      | { initialChildren?: { name: string; value?: string }[] }
      | undefined;
    expect(insert).toBeDefined();
    const children = insert!.initialChildren ?? [];
    const keyChild = children.find((c) => c.name === "key");
    expect(keyChild?.value).toBe("Root");
  });

  it("removes item not present in current list", () => {
    const item = makeItem(10, "Root", "hello");
    const edits = fieldToXmlEdit(makeInput([], [item]));
    const remove = edits.find((e) => e.type === "removeObjectListItem") as
      | { listItemNodeId: number }
      | undefined;
    expect(remove).toBeDefined();
    expect(remove!.listItemNodeId).toBe(10);
  });

  it("updates key via setObjectListItemChildText with childName=key", () => {
    const initial = makeItem(10, "OldKey", "hello");
    const current = { ...initial, className: "NewKey" };
    const edits = fieldToXmlEdit(makeInput([current], [initial]));
    const keyEdit = edits.find(
      (e) => e.type === "setObjectListItemChildText" && (e as { childName?: string }).childName === "key",
    ) as { listItemNodeId: number; childName: string; value: string } | undefined;
    expect(keyEdit).toBeDefined();
    expect(keyEdit!.listItemNodeId).toBe(10);
    expect(keyEdit!.value).toBe("NewKey");
  });

  it("updates scalar field via setNestedElementText with objectPath=['value']", () => {
    const initial = makeItem(10, "Root", "old");
    const current = makeItem(10, "Root", "updated");
    const edits = fieldToXmlEdit(makeInput([current], [initial]));
    const nested = edits.find((e) => e.type === "setNestedElementText") as
      | { parentNodeId: number; objectPath: string[]; fieldName: string; value: string }
      | undefined;
    expect(nested).toBeDefined();
    expect(nested!.parentNodeId).toBe(10);
    expect(nested!.objectPath).toEqual(["value"]);
    expect(nested!.fieldName).toBe("knownField");
    expect(nested!.value).toBe("updated");
  });

  it("emits no edits when nothing changed", () => {
    const item = makeItem(10, "Root", "hello");
    const edits = fieldToXmlEdit(makeInput([item], [item]));
    expect(edits).toEqual([]);
  });
});
