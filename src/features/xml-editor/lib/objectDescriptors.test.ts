import { describe, it, expect } from "vitest";
import {
  getAllObjectFields,
  resolveObjectSchema,
  fieldSchemaToControl,
  buildObjectFieldValue,
  buildKeyedObjectMapItemValue,
} from "./objectDescriptors";
import type { FieldSchema, SchemaCatalog } from "../../schema-catalog";
import type { XmlListItemView, XmlNestedChildView } from "../types/xmlDocument";

function makeField(xml: FieldSchema["xml"], typeKind: FieldSchema["type"]["kind"] = "string"): FieldSchema {
  return {
    type: { kind: typeKind },
    required: false,
    examples: [],
    repeatable: false,
    xml,
    flags: false,
  };
}

const emptyCatalog: SchemaCatalog = { formatVersion: 1, packs: [], defTypes: {}, objectTypes: {} };

describe("getAllObjectFields", () => {
  it("returns own fields when no inherits", () => {
    const catalog: SchemaCatalog = {
      ...emptyCatalog,
      objectTypes: {
        Foo: { fieldOrder: ["a"], fields: { a: makeField("element") } },
      },
    };
    const fields = getAllObjectFields("Foo", catalog);
    expect(fields.has("a")).toBe(true);
    expect(fields.size).toBe(1);
  });

  it("merges inherited fields, inherited first", () => {
    const catalog: SchemaCatalog = {
      ...emptyCatalog,
      objectTypes: {
        Base: { fieldOrder: ["baseField"], fields: { baseField: makeField("element") } },
        Child: {
          fieldOrder: ["ownField"],
          inherits: ["Base"],
          fields: { ownField: makeField("element") },
        },
      },
    };
    const fields = getAllObjectFields("Child", catalog);
    expect(fields.has("baseField")).toBe(true);
    expect(fields.has("ownField")).toBe(true);
    const names = [...fields.keys()];
    expect(names.indexOf("baseField")).toBeLessThan(names.indexOf("ownField"));
  });

  it("does not loop on a cycle in inherits", () => {
    const catalog: SchemaCatalog = {
      ...emptyCatalog,
      objectTypes: {
        A: { fieldOrder: ["x"], inherits: ["B"], fields: { x: makeField("element") } },
        B: { fieldOrder: ["y"], inherits: ["A"], fields: { y: makeField("element") } },
      },
    };
    expect(() => getAllObjectFields("A", catalog)).not.toThrow();
  });
});

describe("resolveObjectSchema", () => {
  it("returns base schema when no discriminator", () => {
    const catalog: SchemaCatalog = {
      ...emptyCatalog,
      objectTypes: { Base: { fieldOrder: [], fields: {} } },
    };
    const result = resolveObjectSchema("Base", "", catalog);
    expect(result.schemaRef).toBe("Base");
    expect(result.schema).not.toBeNull();
  });

  it("returns variant schema when className matches discriminator", () => {
    const catalog: SchemaCatalog = {
      ...emptyCatalog,
      objectTypes: {
        Base: {
          fieldOrder: [],
          fields: {},
          discriminator: {
            attribute: "Class",
            allowMissing: true,
            allowUnknown: false,
            variants: { VariantA: "VariantASchema" },
          },
        },
        VariantASchema: { fieldOrder: [], fields: {} },
      },
    };
    const result = resolveObjectSchema("Base", "VariantA", catalog);
    expect(result.schemaRef).toBe("VariantASchema");
    expect(result.schema).not.toBeNull();
  });

  it("returns base schemaRef for unknown class when allowUnknown = true", () => {
    const catalog: SchemaCatalog = {
      ...emptyCatalog,
      objectTypes: {
        Base: {
          fieldOrder: [],
          fields: {},
          discriminator: {
            attribute: "Class",
            allowMissing: true,
            allowUnknown: true,
            variants: {},
          },
        },
      },
    };
    // Unknown class + allowUnknown → base schema so inherited fields (e.g. Class) remain visible.
    const result = resolveObjectSchema("Base", "SomeMissingClass", catalog);
    expect(result.schemaRef).toBe("Base");
    expect(result.schema).not.toBeNull();
  });

  it("returns null for a missing base schema", () => {
    const result = resolveObjectSchema("DoesNotExist", "", emptyCatalog);
    expect(result.schemaRef).toBeNull();
    expect(result.schema).toBeNull();
  });
});

describe("fieldSchemaToControl", () => {
  it("maps typedReferenceList → typedReferenceList", () => {
    expect(fieldSchemaToControl("x", makeField("typedReferenceList"))).toBe("typedReferenceList");
  });

  it("maps namedChildrenMap → namedMap", () => {
    expect(fieldSchemaToControl("x", makeField("namedChildrenMap"))).toBe("namedMap");
  });

  it("maps keyedValueList → namedMap", () => {
    expect(fieldSchemaToControl("x", makeField("keyedValueList"))).toBe("namedMap");
  });

  it("maps listOfLi with object items → objectList", () => {
    const field: FieldSchema = { ...makeField("listOfLi"), items: { kind: "object", schemaRef: "Foo" } };
    expect(fieldSchemaToControl("x", field)).toBe("objectList");
  });

  it("maps boolean element → checkbox", () => {
    expect(fieldSchemaToControl("x", makeField("element", "boolean"))).toBe("checkbox");
  });

  it("maps integer element → number", () => {
    expect(fieldSchemaToControl("x", makeField("element", "integer"))).toBe("number");
  });

  it("maps defReference element → reference", () => {
    expect(fieldSchemaToControl("x", makeField("element", "defReference"))).toBe("reference");
  });

  it("maps string element → text", () => {
    expect(fieldSchemaToControl("x", makeField("element", "string"))).toBe("text");
  });

  it("maps color element → color", () => {
    expect(fieldSchemaToControl("x", makeField("element", "color"))).toBe("color");
  });
});

describe("buildObjectFieldValue", () => {
  it("returns scalar empty string for a string element with no child", () => {
    const result = buildObjectFieldValue(
      undefined,
      "myField",
      makeField("element", "string"),
      emptyCatalog,
      0,
    );
    expect(result).toEqual({ kind: "scalar", value: "" });
  });

  it("returns objectList for listOfLi with object items", () => {
    const fieldSchema: FieldSchema = {
      ...makeField("listOfLi"),
      items: { kind: "object", schemaRef: "Foo" },
    };
    const catalog: SchemaCatalog = {
      ...emptyCatalog,
      objectTypes: { Foo: { fieldOrder: [], fields: {} } },
    };
    const result = buildObjectFieldValue(undefined, "myField", fieldSchema, catalog, 0);
    expect(result.kind).toBe("objectList");
  });

  it("returns readonly at max depth for a self-recursive objectList schema", () => {
    const fieldSchema: FieldSchema = {
      ...makeField("listOfLi"),
      items: { kind: "object", schemaRef: "Foo" },
    };
    const catalog: SchemaCatalog = {
      ...emptyCatalog,
      objectTypes: { Foo: { fieldOrder: [], fields: {} } },
    };
    const result = buildObjectFieldValue(
      undefined,
      "myField",
      fieldSchema,
      catalog,
      6,
    );
    expect(result.kind).toBe("readonly");
  });

  it("returns list for a plain listOfLi field (no object items)", () => {
    const result = buildObjectFieldValue(
      undefined,
      "tags",
      makeField("listOfLi"),
      emptyCatalog,
      0,
    );
    expect(result).toEqual({ kind: "list", items: [] });
  });
});

function makeNestedChild(
  name: string,
  textValue: string | null,
  children?: XmlNestedChildView[],
): XmlNestedChildView {
  return {
    nodeId: Math.random() * 1000 | 0,
    name,
    textValue,
    listItems: [],
    xmlShape: children ? "object" : "element",
    children,
    order: 0,
    line: null,
    column: null,
  };
}

function makeLiItem(keyText: string, valueChildren: XmlNestedChildView[]): XmlListItemView {
  return {
    nodeId: Math.random() * 1000 | 0,
    textValue: null,
    attributes: [],
    children: [
      makeNestedChild("key", keyText),
      makeNestedChild("value", null, valueChildren),
    ],
    order: 0,
    line: null,
    column: null,
    selfClosing: false,
  };
}

describe("fieldSchemaToControl – keyedObjectMap", () => {
  it("returns objectList when items.schemaRef is present", () => {
    const field: FieldSchema = {
      ...makeField("keyedObjectMap"),
      type: { kind: "list" },
      items: { kind: "object", schemaRef: "PartValue" },
    };
    expect(fieldSchemaToControl("myMap", field)).toBe("objectList");
  });

  it("returns readonlyUnknown when items.schemaRef is absent", () => {
    const field: FieldSchema = {
      ...makeField("keyedObjectMap"),
      type: { kind: "list" },
    };
    expect(fieldSchemaToControl("myMap", field)).toBe("readonlyUnknown");
  });
});

describe("buildKeyedObjectMapItemValue", () => {
  const catalog: SchemaCatalog = {
    formatVersion: 1,
    packs: [],
    defTypes: {},
    objectTypes: {
      PartValue: {
        fieldOrder: ["knownField", "count"],
        fields: {
          knownField: makeField("element", "string"),
          count: makeField("element", "integer"),
        },
      },
    },
  };

  it("reads key from <key> child text", () => {
    const li = makeLiItem("Root", [makeNestedChild("knownField", "hello")]);
    const result = buildKeyedObjectMapItemValue(li, "PartValue", catalog);
    expect(result.className).toBe("Root");
  });

  it("reads value fields from <value> children", () => {
    const li = makeLiItem("Root", [
      makeNestedChild("knownField", "hello"),
      makeNestedChild("count", "5"),
    ]);
    const result = buildKeyedObjectMapItemValue(li, "PartValue", catalog);
    expect(result.fields["knownField"]).toEqual({ kind: "scalar", value: "hello" });
  });

  it("counts unknown <value> children in initialUnknownFieldCount", () => {
    const li = makeLiItem("Root", [
      makeNestedChild("knownField", "hello"),
      makeNestedChild("surprisingField", "??"),
    ]);
    const result = buildKeyedObjectMapItemValue(li, "PartValue", catalog);
    expect(result.initialUnknownFieldCount).toBe(1);
  });

  it("uses empty strings for absent value fields", () => {
    const li = makeLiItem("Root", []);
    const result = buildKeyedObjectMapItemValue(li, "PartValue", catalog);
    expect(result.fields["knownField"]).toEqual({ kind: "scalar", value: "" });
  });

  it("returns null schemaRef when schema is not in catalog", () => {
    const li = makeLiItem("Root", []);
    const result = buildKeyedObjectMapItemValue(li, "MissingSchema", catalog);
    expect(result.schemaRef).toBeNull();
  });
});
