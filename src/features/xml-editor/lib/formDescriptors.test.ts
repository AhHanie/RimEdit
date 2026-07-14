import {
  buildFormDescriptors,
  buildFormFieldModels,
  collectEffectiveTopLevelDefFields,
  collectTopLevelFieldSummaries,
  findFieldSchema,
} from "./formDescriptors";
import type {
  DefEditorView,
  XmlAttributeView,
  XmlListItemView,
  XmlNestedChildView,
} from "../types/xmlDocument";
import type {
  FieldSchema,
  ReferenceMetadata,
  SchemaCatalog,
} from "../../schema-catalog";
import type { ObjectListItemValue } from "../types/editorForm";
import {
  makeNestedVisualCatalog,
  makeNestedVisualDefView,
} from "../__fixtures__/graphicData";

function makeChild(
  name: string,
  textValue: string | null = null,
  xmlShape: DefEditorView["children"][number]["xmlShape"] = "element",
) {
  return {
    nodeId: 1,
    name,
    textValue,
    listItems: [] as string[],
    xmlShape,
    order: 0,
    known: false,
    line: null,
    column: null,
  };
}

function makeDef(children: DefEditorView["children"]): DefEditorView {
  return {
    nodeId: 0,
    defType: "ThingDef",
    defName: "Test",
    label: null,
    parentName: null,
    line: null,
    column: null,
    attributes: [],
    children,
  };
}

function makeField(overrides: Partial<FieldSchema> = {}): FieldSchema {
  return {
    type: { kind: "string" },
    required: false,
    repeatable: false,
    xml: "element",
    examples: [],
    flags: false,
    ...overrides,
  };
}

function makeCatalog(
  defType: string,
  fields: Record<string, FieldSchema>,
  inherits: string[] = [],
): SchemaCatalog {
  return {
    formatVersion: 1,
    packs: [],
    objectTypes: {},
    defTypes: {
      [defType]: {
        inherits,
        abstractType: false,
        fieldOrder: [],
        fields,
      },
    },
  };
}

describe("buildFormDescriptors", () => {
  it("maps string field to text control", () => {
    const catalog = makeCatalog("ThingDef", {
      defName: makeField({ type: { kind: "string" } }),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeChild("defName", "Steel")]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.control).toBe("text");
    expect(desc.key).toBe("defName");
    expect(desc.value).toBe("Steel");
    expect(desc.readonly).toBe(false);
  });

  it("maps boolean field to checkbox control", () => {
    const catalog = makeCatalog("ThingDef", {
      abstract: makeField({ type: { kind: "boolean" } }),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeChild("abstract", "true")]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.control).toBe("checkbox");
  });

  it("maps integer field to number control", () => {
    const catalog = makeCatalog("ThingDef", {
      stackLimit: makeField({ type: { kind: "integer" } }),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeChild("stackLimit", "75")]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.control).toBe("number");
  });

  it("maps description field name to textarea", () => {
    const catalog = makeCatalog("ThingDef", {
      description: makeField({ type: { kind: "string" } }),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeChild("description", "some text")]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.control).toBe("textarea");
  });

  it("maps enum with allowedValues to select control", () => {
    const catalog = makeCatalog("ThingDef", {
      category: makeField({
        type: { kind: "enum" },
        validationHints: { allowedValues: ["A", "B", "C"] },
      }),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeChild("category", "A")]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.control).toBe("select");
    expect(desc.allowedValues).toEqual(["A", "B", "C"]);
  });

  it("maps named child maps to namedMap control and is editable", () => {
    const catalog = makeCatalog("ThingDef", {
      statBases: makeField({
        type: { kind: "statMap" },
        xml: "namedChildrenMap",
      }),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeChild("statBases", null, "object")]);
    const [field] = buildFormFieldModels(def, schema, catalog);
    expect(field.control).toBe("namedMap");
    expect(field.readonly).toBe(false);
    expect(field.path.kind).toBe("namedMap");
  });

  it("maps keyed value lists to namedMap control and is editable", () => {
    const catalog = makeCatalog("ThingDef", {
      skillRequirements: makeField({
        type: { kind: "list" },
        items: { kind: "object", schemaRef: "SkillRequirement" },
        xml: "keyedValueList",
        keyField: "skill",
        valueField: "minLevel",
      }),
    });
    const schema = catalog.defTypes["ThingDef"];
    const skillRequirements: DefEditorView["children"][number] = {
      nodeId: 1,
      name: "skillRequirements",
      textValue: null,
      listItems: [],
      xmlShape: "object",
      children: [makeNestedChild("Crafting", "8")],
      order: 0,
      known: false,
      line: null,
      column: null,
    };
    const def = makeDef([skillRequirements]);
    const [field] = buildFormFieldModels(def, schema, catalog);
    expect(field.control).toBe("namedMap");
    expect(field.readonly).toBe(false);
    expect(field.path.kind).toBe("namedMap");
  });

  it("forwards keyReference from field schema to namedMap model", () => {
    const keyRef: ReferenceMetadata = {
      defType: "SkillDef",
      allowAbstract: false,
      scope: "allSources",
    };
    const catalog = makeCatalog("ThingDef", {
      skillGains: makeField({
        type: { kind: "list" },
        items: { kind: "object", schemaRef: "SkillGain" },
        xml: "keyedValueList",
        keyField: "skill",
        valueField: "amount",
        keyReference: keyRef,
      }),
    });
    const schema = catalog.defTypes["ThingDef"];
    const skillGains: DefEditorView["children"][number] = {
      nodeId: 1,
      name: "skillGains",
      textValue: null,
      listItems: [],
      xmlShape: "object",
      children: [makeNestedChild("Shooting", "4")],
      order: 0,
      known: false,
      line: null,
      column: null,
    };
    const def = makeDef([skillGains]);
    const [field] = buildFormFieldModels(def, schema, catalog);
    expect(field.control).toBe("namedMap");
    expect(field.keyReference).toEqual(keyRef);
  });

  it("produces readonlyUnknown for children not in schema", () => {
    const catalog = makeCatalog("ThingDef", {});
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeChild("weirdCustomField", "42")]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.control).toBe("readonlyUnknown");
    expect(desc.readonly).toBe(true);
  });

  it("produces readonlyUnknown for all fields when schema is null", () => {
    const catalog = makeCatalog("ThingDef", {});
    const def = makeDef([makeChild("defName", "X"), makeChild("label", "x")]);
    const descs = buildFormDescriptors(def, null, catalog);
    expect(descs).toHaveLength(2);
    expect(descs.every((d) => d.control === "readonlyUnknown")).toBe(true);
  });

  it("traverses inherits chain to find parent field", () => {
    const parentField = makeField({
      type: { kind: "boolean" },
      label: "Abstract Flag",
    });
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {},
      defTypes: {
        ThingDef: {
          inherits: ["BaseDef"],
          abstractType: false,
          fieldOrder: [],
          fields: {},
        },
        BaseDef: {
          inherits: [],
          abstractType: true,
          fieldOrder: [],
          fields: { abstract: parentField },
        },
      },
    };
    const schema = catalog.defTypes["ThingDef"];
    const found = findFieldSchema("abstract", schema, catalog);
    expect(found).toBe(parentField);
    expect(found?.type.kind).toBe("boolean");
  });

  it("places ancestor fields before concrete type fields in descriptor order", () => {
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {},
      defTypes: {
        ThingDef: {
          inherits: ["Def"],
          abstractType: false,
          fieldOrder: ["stackLimit"],
          fields: { stackLimit: makeField({ type: { kind: "integer" } }) },
        },
        Def: {
          inherits: [],
          abstractType: true,
          fieldOrder: ["defName", "label"],
          fields: {
            defName: makeField({ type: { kind: "string" } }),
            label: makeField({ type: { kind: "string" } }),
          },
        },
      },
    };
    const schema = catalog.defTypes["ThingDef"]!;
    const def = makeDef([
      makeChild("defName", "X"),
      makeChild("label", "x"),
      makeChild("stackLimit", "10"),
    ]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const keys = descs.map((d) => d.key);
    expect(keys.indexOf("defName")).toBeLessThan(keys.indexOf("stackLimit"));
    expect(keys.indexOf("label")).toBeLessThan(keys.indexOf("stackLimit"));
  });

  it("builds attribute-backed field models", () => {
    const catalog = makeCatalog("ThingDef", {
      ParentName: makeField({ type: { kind: "string" }, xml: "attribute" }),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = {
      ...makeDef([]),
      attributes: [{ name: "ParentName", value: "BaseThing", known: false }],
    };
    const [field] = buildFormFieldModels(def, schema, catalog);
    expect(field.path).toEqual({
      kind: "attribute",
      attributeName: "ParentName",
    });
    expect(field.sourceNodeId).toBe(def.nodeId);
  });

  const scalarRefMeta: ReferenceMetadata = {
    defType: "StatDef",
    allowAbstract: false,
    scope: "allSources",
  };

  it("maps defReference element field to reference control", () => {
    const catalog = makeCatalog("ThingDef", {
      workSpeedStat: makeField({
        type: { kind: "defReference" },
        xml: "element",
        reference: scalarRefMeta,
      }),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeChild("workSpeedStat", "GeneralLaborSpeed")]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.control).toBe("reference");
    expect(desc.readonly).toBe(false);
    expect(desc.reference).toEqual(scalarRefMeta);
  });

  it("maps listOfLi field with reference to list control and is editable", () => {
    const listRefMeta: ReferenceMetadata = {
      defType: "RecipeDef",
      allowAbstract: false,
      scope: "allSources",
    };
    const catalog = makeCatalog("ThingDef", {
      recipes: makeField({
        type: { kind: "defReference" },
        xml: "listOfLi",
        reference: listRefMeta,
      }),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeChild("recipes", null, "listOfLi")]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.control).toBe("list");
    expect(desc.readonly).toBe(false);
    expect(desc.reference).toEqual(listRefMeta);
  });

  it("maps listOfLi field with items.kind=defReference and items.reference to reference-aware list", () => {
    const listRefMeta: ReferenceMetadata = {
      defType: "MemeDef",
      allowAbstract: false,
      scope: "allSources",
    };
    const catalog = makeCatalog("ThingDef", {
      memes: makeField({
        type: { kind: "list" },
        xml: "listOfLi",
        items: { kind: "defReference", reference: listRefMeta },
      }),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeChild("memes", null, "listOfLi")]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.control).toBe("list");
    expect(desc.readonly).toBe(false);
    expect(desc.reference).toEqual(listRefMeta);
  });

  it("maps listOfLi field with items.kind=object and schemaRef to editable objectList", () => {
    const catalog = makeCatalog("ThingDef", {
      costList: makeField({
        type: { kind: "list" },
        xml: "listOfLi",
        items: { kind: "object", schemaRef: "CostItem" },
      }),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeChild("costList", null, "listOfLi")]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.control).toBe("objectList");
    // schemaRef is set → editable, ObjectListEditor renders; schema lookup is runtime
    expect(desc.readonly).toBe(false);
    expect(desc.itemSchemaRef).toBe("CostItem");
    expect(desc.reference).toBeUndefined();
  });

  it("maps listOfLi field with items.kind=object and no schemaRef to readonly objectList", () => {
    const catalog = makeCatalog("ThingDef", {
      costList: makeField({
        type: { kind: "list" },
        xml: "listOfLi",
        items: { kind: "object" },
      }),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeChild("costList", null, "listOfLi")]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.control).toBe("objectList");
    // no schemaRef → still readonly (no way to resolve item schemas)
    expect(desc.readonly).toBe(true);
    expect(desc.itemSchemaRef).toBeUndefined();
  });

  it("propagates reference metadata through buildFormFieldModels", () => {
    const catalog = makeCatalog("ThingDef", {
      workSpeedStat: makeField({
        type: { kind: "defReference" },
        xml: "element",
        reference: scalarRefMeta,
      }),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeChild("workSpeedStat", "GeneralLaborSpeed")]);
    const [model] = buildFormFieldModels(def, schema, catalog);
    expect(model.reference).toEqual(scalarRefMeta);
    expect(model.readonly).toBe(false);
  });
});

// Helpers for nested object tests
function makeNestedChild(
  name: string,
  textValue: string | null = null,
): XmlNestedChildView {
  return {
    nodeId: 11,
    name,
    textValue,
    listItems: [],
    xmlShape: "element",
    order: 0,
    line: null,
    column: null,
  };
}

function makeChildWithNested(name: string, children: XmlNestedChildView[]) {
  return {
    nodeId: 10,
    name,
    textValue: null as string | null,
    listItems: [] as string[],
    xmlShape: "object" as const,
    children,
    order: 0,
    known: false,
    line: null,
    column: null,
  };
}

function makeCatalogWithObjectType(
  defTypeFields: Record<string, FieldSchema>,
  objectTypeName: string,
  objectTypeFields: Record<string, FieldSchema>,
  objectFieldOrder: string[] = [],
): SchemaCatalog {
  return {
    formatVersion: 1,
    packs: [],
    objectTypes: {
      [objectTypeName]: {
        fieldOrder: objectFieldOrder,
        fields: objectTypeFields,
      },
    },
    defTypes: {
      ThingDef: {
        inherits: [],
        abstractType: false,
        fieldOrder: [],
        fields: defTypeFields,
      },
    },
  };
}

describe("buildFormDescriptors – nested object fields", () => {
  it("expands schema-backed object to nested text descriptor", () => {
    const catalog = makeCatalogWithObjectType(
      {
        graphicData: makeField({
          type: { kind: "object", schemaRef: "GraphicData" },
          xml: "object",
        }),
      },
      "GraphicData",
      { texPath: makeField({ type: { kind: "string" }, xml: "element" }) },
    );
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([
      makeChildWithNested("graphicData", [
        makeNestedChild("texPath", "Things/Test"),
      ]),
    ]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "graphicData.texPath");
    expect(d).toBeDefined();
    expect(d!.control).toBe("text");
    expect(d!.value).toBe("Things/Test");
    expect(d!.readonly).toBe(false);
    expect(d!.fieldPath).toEqual(["graphicData", "texPath"]);
  });

  it("maps nested keyed value lists to namedMap control", () => {
    const catalog = makeCatalogWithObjectType(
      {
        recipeMaker: makeField({
          type: { kind: "object", schemaRef: "RecipeMakerProperties" },
          xml: "object",
        }),
      },
      "RecipeMakerProperties",
      {
        skillRequirements: makeField({
          type: { kind: "list" },
          items: { kind: "object", schemaRef: "SkillRequirement" },
          xml: "keyedValueList",
          keyField: "skill",
          valueField: "minLevel",
        }),
      },
      ["skillRequirements"],
    );
    const schema = catalog.defTypes["ThingDef"];
    const skillRequirements: XmlNestedChildView = {
      ...makeNestedChild("skillRequirements", null),
      xmlShape: "object",
      children: [makeNestedChild("Crafting", "8")],
    };
    const def = makeDef([
      makeChildWithNested("recipeMaker", [skillRequirements]),
    ]);
    const [descriptor] = buildFormDescriptors(def, schema, catalog);
    const [field] = buildFormFieldModels(def, schema, catalog);
    expect(descriptor.control).toBe("namedMap");
    expect(descriptor.value).toEqual([{ key: "Crafting", value: "8" }]);
    expect(field.path).toEqual({
      kind: "namedMap",
      objectPath: ["recipeMaker"],
      mapName: "skillRequirements",
    });
  });

  it("expands schema-backed object to select when allowedValues present", () => {
    const catalog = makeCatalogWithObjectType(
      {
        graphicData: makeField({
          type: { kind: "object", schemaRef: "GraphicData" },
          xml: "object",
        }),
      },
      "GraphicData",
      {
        graphicClass: makeField({
          type: { kind: "enum" },
          xml: "element",
          validationHints: {
            allowedValues: ["Graphic_Single", "Graphic_Multi"],
          },
        }),
      },
    );
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeChildWithNested("graphicData", [])]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "graphicData.graphicClass");
    expect(d?.control).toBe("select");
    expect(d?.allowedValues).toEqual(["Graphic_Single", "Graphic_Multi"]);
  });

  it("creates descriptor with empty value for missing nested scalar under existing object", () => {
    const catalog = makeCatalogWithObjectType(
      {
        graphicData: makeField({
          type: { kind: "object", schemaRef: "GraphicData" },
          xml: "object",
        }),
      },
      "GraphicData",
      { graphicClass: makeField({ type: { kind: "string" }, xml: "element" }) },
    );
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeChildWithNested("graphicData", [])]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "graphicData.graphicClass");
    expect(d).toBeDefined();
    expect(d!.value).toBe("");
    expect(d!.nodeId).toBeNull();
    expect(d!.fieldPath).toEqual(["graphicData", "graphicClass"]);
    expect(d!.readonly).toBe(false);
  });

  it("makes deeper schema-backed nested scalars editable", () => {
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      defTypes: {
        ThingDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: [],
          fields: {
            graphicData: makeField({
              type: { kind: "object", schemaRef: "GraphicData" },
              xml: "object",
            }),
          },
        },
      },
      objectTypes: {
        GraphicData: {
          fieldOrder: [],
          fields: {
            shadowData: makeField({
              type: { kind: "object", schemaRef: "ShadowData" },
              xml: "element",
            }),
          },
        },
        ShadowData: {
          fieldOrder: [],
          fields: {
            volume: makeField({ type: { kind: "float" }, xml: "element" }),
          },
        },
      },
    };
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeChildWithNested("graphicData", [])]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "graphicData.shadowData.volume");
    expect(d).toBeDefined();
    expect(d!.fieldPath).toEqual(["graphicData", "shadowData", "volume"]);
    expect(d!.readonly).toBe(false);
  });

  it("sets nestedObjectField path on model for one-level nested field", () => {
    const catalog = makeCatalogWithObjectType(
      {
        graphicData: makeField({
          type: { kind: "object", schemaRef: "GraphicData" },
          xml: "object",
        }),
      },
      "GraphicData",
      { texPath: makeField({ type: { kind: "string" }, xml: "element" }) },
    );
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([
      makeChildWithNested("graphicData", [
        makeNestedChild("texPath", "Things/Test"),
      ]),
    ]);
    const models = buildFormFieldModels(def, schema, catalog);
    const m = models.find((x) => x.key === "graphicData.texPath");
    expect(m).toBeDefined();
    expect(m!.path).toEqual({
      kind: "nestedObjectField",
      objectPath: ["graphicData"],
      fieldName: "texPath",
    });
    expect(m!.fieldPath).toEqual(["graphicData", "texPath"]);
    expect(m!.readonly).toBe(false);
  });

  it("makes nested scalar fields editable even when the parent object element is absent from XML", () => {
    const catalog = makeCatalogWithObjectType(
      {
        graphicData: makeField({
          type: { kind: "object", schemaRef: "GraphicData" },
          xml: "object",
        }),
      },
      "GraphicData",
      { texPath: makeField({ type: { kind: "string" }, xml: "element" }) },
    );
    const schema = catalog.defTypes["ThingDef"];
    // No graphicData child in XML at all - backend will create it
    const def = makeDef([]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "graphicData.texPath");
    expect(d).toBeDefined();
    expect(d!.readonly).toBe(false);
    expect(d!.value).toBe("");
    expect(d!.nodeId).toBeNull();
    expect(d!.fieldPath).toEqual(["graphicData", "texPath"]);
  });

  it("renders nested reference-list fields as editable list control", () => {
    const catalog = makeCatalogWithObjectType(
      {
        graphicData: makeField({
          type: { kind: "object", schemaRef: "GraphicData" },
          xml: "object",
        }),
      },
      "GraphicData",
      {
        recipes: makeField({
          type: { kind: "defReference" },
          xml: "listOfLi",
          reference: {
            defType: "RecipeDef",
            allowAbstract: false,
            scope: "allSources",
          },
        }),
      },
    );
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeChildWithNested("graphicData", [])]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "graphicData.recipes");
    expect(d).toBeDefined();
    expect(d!.control).toBe("list");
    expect(d!.readonly).toBe(false);
  });

  it("marks nested attribute-shaped fields as editable with nestedAttribute path", () => {
    const catalog = makeCatalogWithObjectType(
      {
        graphicData: makeField({
          type: { kind: "object", schemaRef: "GraphicData" },
          xml: "object",
        }),
      },
      "GraphicData",
      { color: makeField({ type: { kind: "string" }, xml: "attribute" }) },
    );
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeChildWithNested("graphicData", [])]);
    const models = buildFormFieldModels(def, schema, catalog);
    const m = models.find((x) => x.key === "graphicData.color");
    expect(m).toBeDefined();
    expect(m!.readonly).toBe(false);
    expect(m!.path.kind).toBe("nestedAttribute");
  });

  it("does not produce a separate object descriptor for a schema-backed object field", () => {
    const catalog = makeCatalogWithObjectType(
      {
        graphicData: makeField({
          type: { kind: "object", schemaRef: "GraphicData" },
          xml: "object",
        }),
      },
      "GraphicData",
      { texPath: makeField({ type: { kind: "string" }, xml: "element" }) },
    );
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([
      makeChildWithNested("graphicData", [makeNestedChild("texPath", "x")]),
    ]);
    const descs = buildFormDescriptors(def, schema, catalog);
    // No descriptor with key "graphicData" (the object placeholder should be suppressed)
    expect(descs.find((x) => x.key === "graphicData")).toBeUndefined();
    expect(descs.find((x) => x.key === "graphicData.texPath")).toBeDefined();
  });
});

// --- Nested section schema coverage tests ---

function makeNestedSectionCatalog(): SchemaCatalog {
  return {
    formatVersion: 1,
    packs: [],
    defTypes: {
      TestDef: {
        inherits: [],
        abstractType: false,
        fieldOrder: [],
        fields: {
          section: makeField({
            type: { kind: "object", schemaRef: "SectionType" },
            xml: "object",
          }),
        },
      },
    },
    objectTypes: {
      SectionType: {
        fieldOrder: [],
        fields: {
          textField: makeField({ type: { kind: "string" }, xml: "element" }),
          enumField: makeField({
            type: { kind: "enum" },
            xml: "element",
            validationHints: {
              allowedValues: ["Option_A", "Option_B", "Option_C"],
            },
          }),
          modeField: makeField({
            type: { kind: "enum" },
            xml: "element",
            validationHints: {
              allowedValues: [
                "Mode_None",
                "Mode_Basic",
                "Mode_Corner",
                "Mode_Link",
                "Mode_Overlay",
                "Mode_Trans",
                "Mode_Asym",
              ],
            },
          }),
          namedMapField: makeField({
            type: { kind: "object" },
            xml: "namedChildrenMap",
          }),
          itemListA: makeField({
            type: { kind: "list" },
            xml: "listOfLi",
            items: { kind: "object", schemaRef: "ItemTypeA" },
          }),
          itemListB: makeField({
            type: { kind: "list" },
            xml: "listOfLi",
            items: { kind: "object", schemaRef: "SectionType" },
          }),
          flagsField: makeField({
            type: { kind: "list" },
            xml: "listOfLi",
            items: { kind: "enum" },
            flags: true,
            validationHints: {
              allowedValues: ["Flag_None", "Flag_A", "Flag_B", "Flag_C"],
            },
          }),
          subSectionA: makeField({
            type: { kind: "object", schemaRef: "SubSectionA" },
            xml: "object",
          }),
          subSectionB: makeField({
            type: { kind: "object", schemaRef: "SubSectionB" },
            xml: "object",
          }),
        },
      },
      SubSectionA: {
        fieldOrder: [],
        fields: {
          vec3A: makeField({ type: { kind: "vector3" }, xml: "element" }),
          vec3B: makeField({ type: { kind: "vector3" }, xml: "element" }),
        },
      },
      SubSectionB: {
        fieldOrder: [],
        fields: {
          strFieldB: makeField({ type: { kind: "string" }, xml: "element" }),
          boolFieldB: makeField({ type: { kind: "boolean" }, xml: "element" }),
        },
      },
    },
  };
}

describe("buildFormDescriptors – nested section schema coverage", () => {
  it("expands section.textField as editable text", () => {
    const catalog = makeNestedSectionCatalog();
    const schema = catalog.defTypes["TestDef"];
    const def = makeDef([
      makeChildWithNested("section", [
        makeNestedChild("textField", "some-value"),
      ]),
    ]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "section.textField");
    expect(d).toBeDefined();
    expect(d!.control).toBe("text");
    expect(d!.readonly).toBe(false);
    expect(d!.value).toBe("some-value");
  });

  it("renders section.enumField as a select control", () => {
    const catalog = makeNestedSectionCatalog();
    const schema = catalog.defTypes["TestDef"];
    const def = makeDef([
      makeChildWithNested("section", [
        makeNestedChild("enumField", "Option_A"),
      ]),
    ]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "section.enumField");
    expect(d).toBeDefined();
    expect(d!.control).toBe("select");
    expect(d!.allowedValues).toContain("Option_A");
    expect(d!.allowedValues).toContain("Option_B");
    expect(d!.allowedValues).toContain("Option_C");
    expect(d!.readonly).toBe(false);
  });

  it("renders section.modeField as a select control", () => {
    const catalog = makeNestedSectionCatalog();
    const schema = catalog.defTypes["TestDef"];
    const def = makeDef([makeChildWithNested("section", [])]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "section.modeField");
    expect(d).toBeDefined();
    expect(d!.control).toBe("select");
    expect(d!.allowedValues).toContain("Mode_None");
    expect(d!.allowedValues).toContain("Mode_Asym");
  });

  it("expands section.subSectionA.vec3A as editable nested field", () => {
    const catalog = makeNestedSectionCatalog();
    const schema = catalog.defTypes["TestDef"];
    const def = makeDef([makeChildWithNested("section", [])]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "section.subSectionA.vec3A");
    expect(d).toBeDefined();
    expect(d!.fieldPath).toEqual(["section", "subSectionA", "vec3A"]);
    expect(d!.readonly).toBe(false);
  });

  it("expands section.subSectionB.strFieldB as editable nested field", () => {
    const catalog = makeNestedSectionCatalog();
    const schema = catalog.defTypes["TestDef"];
    const def = makeDef([makeChildWithNested("section", [])]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "section.subSectionB.strFieldB");
    expect(d).toBeDefined();
    expect(d!.fieldPath).toEqual(["section", "subSectionB", "strFieldB"]);
    expect(d!.readonly).toBe(false);
  });

  it("marks section.namedMapField as namedMap (editable key/value)", () => {
    const catalog = makeNestedSectionCatalog();
    const schema = catalog.defTypes["TestDef"];
    const def = makeDef([makeChildWithNested("section", [])]);
    const models = buildFormFieldModels(def, schema, catalog);
    const m = models.find((x) => x.key === "section.namedMapField");
    expect(m).toBeDefined();
    expect(m!.control).toBe("namedMap");
    expect(m!.readonly).toBe(false);
  });

  it("marks section.itemListA as objectList (editable when schemaRef present)", () => {
    const catalog = makeNestedSectionCatalog();
    const schema = catalog.defTypes["TestDef"];
    const def = makeDef([makeChildWithNested("section", [])]);
    const models = buildFormFieldModels(def, schema, catalog);
    const m = models.find((x) => x.key === "section.itemListA");
    expect(m).toBeDefined();
    expect(m!.control).toBe("objectList");
    expect(m!.readonly).toBe(false);
  });

  it("marks section.itemListB as objectList (editable when schemaRef present)", () => {
    const catalog = makeNestedSectionCatalog();
    const schema = catalog.defTypes["TestDef"];
    const def = makeDef([makeChildWithNested("section", [])]);
    const models = buildFormFieldModels(def, schema, catalog);
    const m = models.find((x) => x.key === "section.itemListB");
    expect(m).toBeDefined();
    expect(m!.control).toBe("objectList");
    expect(m!.readonly).toBe(false);
  });

  it("renders section.flagsField as flags control (editable multi-select)", () => {
    const catalog = makeNestedSectionCatalog();
    const schema = catalog.defTypes["TestDef"];
    const def = makeDef([makeChildWithNested("section", [])]);
    const models = buildFormFieldModels(def, schema, catalog);
    const m = models.find((x) => x.key === "section.flagsField");
    expect(m).toBeDefined();
    expect(m!.control).toBe("flags");
    expect(m!.readonly).toBe(false);
  });
});

// --- Fixture-backed NestedVisual descriptor acceptance tests ---

describe("buildFormDescriptors – fixture-backed NestedVisual", () => {
  it("expands full NestedVisual fixture into editable nested fields", () => {
    const catalog = makeNestedVisualCatalog();
    const schema = catalog.defTypes["TestDef"];
    const def = makeNestedVisualDefView();
    const descs = buildFormDescriptors(def, schema, catalog);

    const expectedNestedKeys = [
      "visualData.imagePath",
      "visualData.renderStyle",
      "visualData.shadowSection.volume",
      "visualData.shadowSection.offset",
      "visualData.damageSection.enabled",
      "visualData.linkMode",
      "visualData.linkFlags",
      "visualData.shaderParams",
    ];
    for (const key of expectedNestedKeys) {
      const d = descs.find((x) => x.key === key);
      expect(d).toBeDefined();
      expect(d!.readonly).toBe(false);
    }

    const attachments = descs.find((x) => x.key === "visualData.attachments");
    expect(attachments).toBeDefined();
    expect(attachments!.control).toBe("objectList");
    expect(attachments!.readonly).toBe(false);

    const attachPoints = descs.find((x) => x.key === "visualData.attachPoints");
    expect(attachPoints).toBeDefined();
    expect(attachPoints!.control).toBe("objectList");
    expect(attachPoints!.readonly).toBe(false);
  });

  it("renderStyle uses select with schema allowed values from fixture catalog", () => {
    const catalog = makeNestedVisualCatalog();
    const schema = catalog.defTypes["TestDef"];
    const def = makeNestedVisualDefView();
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "visualData.renderStyle");
    expect(d).toBeDefined();
    expect(d!.control).toBe("select");
    expect(d!.allowedValues).toContain("Single");
    expect(d!.allowedValues).toContain("Multi");
    expect(d!.allowedValues).toContain("Random");
    expect(d!.allowedValues).toContain("StackCount");
    expect(d!.readonly).toBe(false);
  });
});

// --- Section metadata (defaultCollapsed / sectionHasData) tests ---

describe("buildFormDescriptors – section metadata", () => {
  function makeSectionCatalog(
    topLevelDefaultCollapsed?: boolean,
    nestedDefaultCollapsed?: boolean,
  ): SchemaCatalog {
    return {
      formatVersion: 1,
      packs: [],
      defTypes: {
        ThingDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: [],
          fields: {
            graphicData: makeField({
              type: { kind: "object", schemaRef: "GraphicData" },
              xml: "object",
              defaultCollapsed: topLevelDefaultCollapsed,
            }),
          },
        },
      },
      objectTypes: {
        GraphicData: {
          fieldOrder: [],
          fields: {
            texPath: makeField({ type: { kind: "string" }, xml: "element" }),
            shadowData: makeField({
              type: { kind: "object", schemaRef: "ShadowData" },
              xml: "object",
              defaultCollapsed: nestedDefaultCollapsed,
            }),
          },
        },
        ShadowData: {
          fieldOrder: [],
          fields: {
            volume: makeField({ type: { kind: "float" }, xml: "element" }),
          },
        },
      },
    };
  }

  it("carries explicit defaultCollapsed: true to top-level section fields", () => {
    const catalog = makeSectionCatalog(true, undefined);
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const texDesc = descs.find((x) => x.key === "graphicData.texPath");
    expect(texDesc).toBeDefined();
    expect(texDesc!.sectionDefaults[0]?.defaultCollapsed).toBe(true);
  });

  it("carries explicit defaultCollapsed: false to top-level section fields", () => {
    const catalog = makeSectionCatalog(false, undefined);
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const texDesc = descs.find((x) => x.key === "graphicData.texPath");
    expect(texDesc).toBeDefined();
    expect(texDesc!.sectionDefaults[0]?.defaultCollapsed).toBe(false);
  });

  it("carries explicit defaultCollapsed: true to nested subsection fields", () => {
    const catalog = makeSectionCatalog(undefined, true);
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const volumeDesc = descs.find(
      (x) => x.key === "graphicData.shadowData.volume",
    );
    expect(volumeDesc).toBeDefined();
    // sectionDefaults[0] = graphicData, sectionDefaults[1] = graphicData.shadowData
    expect(volumeDesc!.sectionDefaults[1]?.defaultCollapsed).toBe(true);
  });

  it("sets sectionHasData: false when top-level object element is absent", () => {
    const catalog = makeSectionCatalog(undefined, undefined);
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([]); // no graphicData child
    const descs = buildFormDescriptors(def, schema, catalog);
    const texDesc = descs.find((x) => x.key === "graphicData.texPath");
    expect(texDesc).toBeDefined();
    expect(texDesc!.sectionDefaults[0]?.hasData).toBe(false);
  });

  it("sets sectionHasData: true when object element is present with text child", () => {
    const catalog = makeSectionCatalog(undefined, undefined);
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([
      makeChildWithNested("graphicData", [
        makeNestedChild("texPath", "Things/Test"),
      ]),
    ]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const texDesc = descs.find((x) => x.key === "graphicData.texPath");
    expect(texDesc).toBeDefined();
    expect(texDesc!.sectionDefaults[0]?.hasData).toBe(true);
  });

  it("sets sectionHasData: false for nested subsection when sub-element is absent", () => {
    const catalog = makeSectionCatalog(undefined, undefined);
    const schema = catalog.defTypes["ThingDef"];
    // graphicData present but no shadowData inside
    const def = makeDef([
      makeChildWithNested("graphicData", [
        makeNestedChild("texPath", "Things/Test"),
      ]),
    ]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const volumeDesc = descs.find(
      (x) => x.key === "graphicData.shadowData.volume",
    );
    expect(volumeDesc).toBeDefined();
    // sectionDefaults[1] = graphicData.shadowData (absent → hasData false)
    expect(volumeDesc!.sectionDefaults[1]?.hasData).toBe(false);
  });

  it("propagates sectionDefaults through buildFormFieldModels", () => {
    const catalog = makeSectionCatalog(true, undefined);
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([]);
    const models = buildFormFieldModels(def, schema, catalog);
    const texModel = models.find((x) => x.key === "graphicData.texPath");
    expect(texModel).toBeDefined();
    expect(texModel!.sectionDefaults[0]?.defaultCollapsed).toBe(true);
    expect(texModel!.sectionDefaults[0]?.hasData).toBe(false);
  });

  it("top-level scalar fields have empty sectionDefaults", () => {
    const catalog = makeCatalog("ThingDef", {
      defName: makeField({ type: { kind: "string" } }),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeChild("defName", "Steel")]);
    const descs = buildFormDescriptors(def, schema, catalog);
    expect(descs[0].sectionDefaults).toEqual([]);
  });
});

// --- Multi-level inherited field descriptor tests ---

function makeMultiLevelInheritanceCatalog(): SchemaCatalog {
  return {
    formatVersion: 1,
    packs: [],
    defTypes: {
      DerivedDef: {
        inherits: ["BaseDef"],
        abstractType: false,
        fieldOrder: ["ownField"],
        fields: {
          ownField: makeField({ type: { kind: "string" }, xml: "element" }),
        },
      },
      BaseDef: {
        inherits: ["Def"],
        abstractType: true,
        fieldOrder: [
          "statMap",
          "layerField",
          "blockedList",
          "costConfig",
          "variantList",
          "iconList",
          "colorField",
        ],
        fields: {
          statMap: makeField({
            type: { kind: "statMap" },
            xml: "namedChildrenMap",
          }),
          layerField: makeField({
            type: { kind: "enum" },
            xml: "element",
            validationHints: {
              allowedValues: [
                "Level_Low",
                "Level_Mid",
                "Level_High",
                "Level_Top",
              ],
            },
          }),
          blockedList: makeField({
            type: { kind: "list" },
            xml: "listOfLi",
            items: { kind: "enum" },
            validationHints: {
              allowedValues: ["Level_Low", "Level_Mid", "Level_High"],
            },
          }),
          costConfig: makeField({
            type: { kind: "object", schemaRef: "CostConfig" },
            xml: "object",
          }),
          variantList: makeField({
            type: { kind: "list" },
            xml: "listOfLi",
            items: { kind: "object", schemaRef: "VariantEntry" },
          }),
          iconList: makeField({
            type: { kind: "list" },
            xml: "listOfLi",
            items: { kind: "object", schemaRef: "IconEntry" },
          }),
          colorField: makeField({ type: { kind: "string" }, xml: "element" }),
        },
      },
      Def: {
        inherits: [],
        abstractType: true,
        fieldOrder: ["defName", "label"],
        fields: {
          defName: makeField({ type: { kind: "string" }, xml: "element" }),
          label: makeField({ type: { kind: "string" }, xml: "element" }),
        },
      },
    },
    objectTypes: {
      CostConfig: {
        fieldOrder: ["configKey", "costMap", "itemCount", "isInverted"],
        fields: {
          configKey: makeField({ type: { kind: "string" }, xml: "element" }),
          costMap: makeField({
            type: { kind: "object" },
            xml: "namedChildrenMap",
          }),
          itemCount: makeField({ type: { kind: "integer" }, xml: "element" }),
          isInverted: makeField({ type: { kind: "boolean" }, xml: "element" }),
        },
      },
      VariantEntry: {
        fieldOrder: ["sourceRef", "colorValue"],
        fields: {
          sourceRef: makeField({
            type: { kind: "defReference" },
            xml: "element",
            reference: {
              defType: "EntityDef",
              allowAbstract: false,
              scope: "allSources",
            },
          }),
          colorValue: makeField({ type: { kind: "string" }, xml: "element" }),
        },
      },
      IconEntry: {
        fieldOrder: ["appearanceRef", "iconPath"],
        fields: {
          appearanceRef: makeField({
            type: { kind: "defReference" },
            xml: "element",
            reference: {
              defType: "AppearanceDef",
              allowAbstract: false,
              scope: "allSources",
            },
          }),
          iconPath: makeField({ type: { kind: "string" }, xml: "element" }),
        },
      },
    },
  };
}

describe("buildFormDescriptors – multi-level inherited fields", () => {
  it("layerField inherited from BaseDef renders as select with allowed values", () => {
    const catalog = makeMultiLevelInheritanceCatalog();
    const schema = catalog.defTypes["DerivedDef"];
    const def = makeDef([makeChild("layerField", "Level_Mid")]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "layerField");
    expect(d).toBeDefined();
    expect(d!.control).toBe("select");
    expect(d!.allowedValues).toContain("Level_Mid");
    expect(d!.allowedValues).toContain("Level_High");
    expect(d!.readonly).toBe(false);
    expect(d!.value).toBe("Level_Mid");
  });

  it("blockedList inherited from BaseDef renders as editable list", () => {
    const catalog = makeMultiLevelInheritanceCatalog();
    const schema = catalog.defTypes["DerivedDef"];
    const def = makeDef([makeChild("blockedList", null, "listOfLi")]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "blockedList");
    expect(d).toBeDefined();
    expect(d!.control).toBe("list");
    expect(d!.readonly).toBe(false);
  });

  it("statMap inherited from BaseDef renders as editable namedMap", () => {
    const catalog = makeMultiLevelInheritanceCatalog();
    const schema = catalog.defTypes["DerivedDef"];
    const def = makeDef([makeChild("statMap", null, "object")]);
    const models = buildFormFieldModels(def, schema, catalog);
    const m = models.find((x) => x.key === "statMap");
    expect(m).toBeDefined();
    expect(m!.control).toBe("namedMap");
    expect(m!.readonly).toBe(false);
    expect(m!.path.kind).toBe("namedMap");
  });

  it("costConfig expands to nested descriptors with costMap as editable namedMap", () => {
    const catalog = makeMultiLevelInheritanceCatalog();
    const schema = catalog.defTypes["DerivedDef"];
    const def = makeDef([]);
    const descs = buildFormDescriptors(def, schema, catalog);

    const configKey = descs.find((x) => x.key === "costConfig.configKey");
    expect(configKey).toBeDefined();
    expect(configKey!.control).toBe("text");
    expect(configKey!.readonly).toBe(false);

    const costMap = descs.find((x) => x.key === "costConfig.costMap");
    expect(costMap).toBeDefined();
    expect(costMap!.control).toBe("namedMap");
    expect(costMap!.readonly).toBe(false);

    const isInverted = descs.find((x) => x.key === "costConfig.isInverted");
    expect(isInverted).toBeDefined();
    expect(isInverted!.control).toBe("checkbox");
    expect(isInverted!.readonly).toBe(false);
  });

  it("costConfig.costMap has namedMap path nested inside costConfig", () => {
    const catalog = makeMultiLevelInheritanceCatalog();
    const schema = catalog.defTypes["DerivedDef"];
    const def = makeDef([]);
    const models = buildFormFieldModels(def, schema, catalog);
    const m = models.find((x) => x.key === "costConfig.costMap");
    expect(m).toBeDefined();
    expect(m!.path).toEqual({
      kind: "namedMap",
      objectPath: ["costConfig"],
      mapName: "costMap",
    });
    expect(m!.readonly).toBe(false);
  });

  it("variantList renders as editable objectList with VariantEntry schemaRef", () => {
    const catalog = makeMultiLevelInheritanceCatalog();
    const schema = catalog.defTypes["DerivedDef"];
    const def = makeDef([makeChild("variantList", null, "listOfLi")]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "variantList");
    expect(d).toBeDefined();
    expect(d!.control).toBe("objectList");
    expect(d!.readonly).toBe(false);
    expect(d!.itemSchemaRef).toBe("VariantEntry");
  });

  it("iconList renders as editable objectList with IconEntry schemaRef", () => {
    const catalog = makeMultiLevelInheritanceCatalog();
    const schema = catalog.defTypes["DerivedDef"];
    const def = makeDef([makeChild("iconList", null, "listOfLi")]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "iconList");
    expect(d).toBeDefined();
    expect(d!.control).toBe("objectList");
    expect(d!.readonly).toBe(false);
    expect(d!.itemSchemaRef).toBe("IconEntry");
  });

  it("colorField renders as editable text field", () => {
    const catalog = makeMultiLevelInheritanceCatalog();
    const schema = catalog.defTypes["DerivedDef"];
    const def = makeDef([makeChild("colorField", "white")]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "colorField");
    expect(d).toBeDefined();
    expect(d!.control).toBe("text");
    expect(d!.readonly).toBe(false);
    expect(d!.value).toBe("white");
  });

  it("Def fields appear before BaseDef fields before DerivedDef own fields", () => {
    const catalog = makeMultiLevelInheritanceCatalog();
    const schema = catalog.defTypes["DerivedDef"];
    const def = makeDef([
      makeChild("defName", "TestInstance"),
      makeChild("layerField", "Level_Mid"),
      makeChild("ownField", "custom"),
    ]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const defNameIdx = descs.findIndex((x) => x.key === "defName");
    const layerIdx = descs.findIndex((x) => x.key === "layerField");
    const ownIdx = descs.findIndex((x) => x.key === "ownField");
    expect(defNameIdx).toBeGreaterThanOrEqual(0);
    expect(layerIdx).toBeGreaterThanOrEqual(0);
    expect(ownIdx).toBeGreaterThanOrEqual(0);
    expect(defNameIdx).toBeLessThan(layerIdx);
    expect(layerIdx).toBeLessThan(ownIdx);
  });

  it("schema-backed BaseDef fields are not readonlyUnknown; only truly unknown fields are", () => {
    const catalog = makeMultiLevelInheritanceCatalog();
    const schema = catalog.defTypes["DerivedDef"];
    const def = makeDef([
      makeChild("layerField", "Level_Mid"),
      makeChild("colorField", "white"),
      makeChild("unknownModField", "42"),
    ]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const unknown = descs.filter((x) => x.control === "readonlyUnknown");
    expect(unknown).toHaveLength(1);
    expect(unknown[0].key).toBe("unknownModField");
  });
});

// --- ThingDef schema-specific shape descriptor tests (Issue 05 / 06) ---

describe("buildFormDescriptors – intRange / floatRange field types", () => {
  it("intRange field maps to text control and is editable", () => {
    const catalog = makeCatalog("ThingDef", {
      deepLumpSizeRange: makeField({
        type: { kind: "intRange" },
        xml: "element",
      }),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeChild("deepLumpSizeRange", "5~10")]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.control).toBe("text");
    expect(desc.readonly).toBe(false);
    expect(desc.value).toBe("5~10");
  });

  it("floatRange field maps to text control and is editable", () => {
    const catalog = makeCatalog("ThingDef", {
      startingHpRange: makeField({
        type: { kind: "floatRange" },
        xml: "element",
      }),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeChild("startingHpRange", "0.9~1.0")]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.control).toBe("text");
    expect(desc.readonly).toBe(false);
    expect(desc.value).toBe("0.9~1.0");
  });

  it("intRange field with no XML value defaults to empty string", () => {
    const catalog = makeCatalog("ThingDef", {
      deepLumpSizeRange: makeField({
        type: { kind: "intRange" },
        xml: "element",
      }),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.control).toBe("text");
    expect(desc.value).toBe("");
    expect(desc.readonly).toBe(false);
  });
});

describe("buildFormDescriptors – flagsText XML shape", () => {
  it("flagsText with allowedValues maps to flags control and parses value as array", () => {
    const catalog = makeCatalog("ThingDef", {
      developmentalStageFilter: makeField({
        type: { kind: "string" },
        xml: "flagsText",
        validationHints: {
          allowedValues: ["Newborn", "Baby", "Child", "Adult"],
        },
      }),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([
      makeChild("developmentalStageFilter", "Child, Adult", "element"),
    ]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.control).toBe("flags");
    expect(desc.readonly).toBe(false);
    expect(desc.value).toEqual(["Child", "Adult"]);
  });

  it("flagsText with allowedValues and absent field defaults to empty array", () => {
    const catalog = makeCatalog("ThingDef", {
      developmentalStageFilter: makeField({
        type: { kind: "string" },
        xml: "flagsText",
        validationHints: {
          allowedValues: ["Newborn", "Baby", "Child", "Adult"],
        },
      }),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.control).toBe("flags");
    expect(desc.readonly).toBe(false);
    expect(desc.value).toEqual([]);
  });

  it("flagsText without allowedValues keeps text control", () => {
    const catalog = makeCatalog("ThingDef", {
      developmentalStageFilter: makeField({
        type: { kind: "string" },
        xml: "flagsText",
      }),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([
      makeChild("developmentalStageFilter", "SomeValue", "element"),
    ]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.control).toBe("text");
    expect(desc.value).toBe("SomeValue");
  });

  it("flagsText with allowedValues uses childElement path (not listItems)", () => {
    const catalog = makeCatalog("ThingDef", {
      developmentalStageFilter: makeField({
        type: { kind: "string" },
        xml: "flagsText",
        validationHints: {
          allowedValues: ["None", "Newborn", "Baby", "Child", "Adult"],
        },
      }),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([
      makeChild("developmentalStageFilter", "Child, Adult", "element"),
    ]);
    const [model] = buildFormFieldModels(def, schema, catalog);
    expect(model.path.kind).toBe("childElement");
  });
});

describe("buildFormDescriptors – multiple object-list fields with stat map", () => {
  function makeObjectListCatalog(): SchemaCatalog {
    return {
      formatVersion: 1,
      packs: [],
      defTypes: {
        TestDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: ["actionList", "toolList", "statOffsets"],
          fields: {
            actionList: makeField({
              type: { kind: "list" },
              xml: "listOfLi",
              items: { kind: "object", schemaRef: "ActionParams" },
            }),
            toolList: makeField({
              type: { kind: "list" },
              xml: "listOfLi",
              items: { kind: "object", schemaRef: "ActionTool" },
            }),
            statOffsets: makeField({
              type: { kind: "statMap" },
              xml: "namedChildrenMap",
            }),
          },
        },
      },
      objectTypes: {
        ActionParams: {
          fieldOrder: ["range", "label"],
          fields: {
            range: makeField({ type: { kind: "float" }, xml: "element" }),
            label: makeField({ type: { kind: "string" }, xml: "element" }),
          },
        },
        ActionTool: {
          fieldOrder: ["label", "power", "cooldownTime"],
          fields: {
            label: makeField({ type: { kind: "string" }, xml: "element" }),
            power: makeField({ type: { kind: "float" }, xml: "element" }),
            cooldownTime: makeField({
              type: { kind: "float" },
              xml: "element",
            }),
          },
        },
      },
    };
  }

  it("toolList field produces editable objectList descriptor with ActionTool schemaRef", () => {
    const catalog = makeObjectListCatalog();
    const schema = catalog.defTypes["TestDef"];
    const def = makeDef([makeChild("toolList", null, "listOfLi")]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "toolList");
    expect(d).toBeDefined();
    expect(d!.control).toBe("objectList");
    expect(d!.readonly).toBe(false);
    expect(d!.itemSchemaRef).toBe("ActionTool");
  });

  it("actionList field produces editable objectList descriptor with ActionParams schemaRef", () => {
    const catalog = makeObjectListCatalog();
    const schema = catalog.defTypes["TestDef"];
    const def = makeDef([makeChild("actionList", null, "listOfLi")]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "actionList");
    expect(d).toBeDefined();
    expect(d!.control).toBe("objectList");
    expect(d!.readonly).toBe(false);
    expect(d!.itemSchemaRef).toBe("ActionParams");
  });

  it("statOffsets produces editable namedMap descriptor", () => {
    const catalog = makeObjectListCatalog();
    const schema = catalog.defTypes["TestDef"];
    const def = makeDef([makeChild("statOffsets", null, "object")]);
    const models = buildFormFieldModels(def, schema, catalog);
    const m = models.find((x) => x.key === "statOffsets");
    expect(m).toBeDefined();
    expect(m!.control).toBe("namedMap");
    expect(m!.readonly).toBe(false);
    expect(m!.path.kind).toBe("namedMap");
  });
});

describe("buildFormDescriptors – collapsed object section with nested list", () => {
  function makeCollapsedSectionCatalog(): SchemaCatalog {
    return {
      formatVersion: 1,
      packs: [],
      defTypes: {
        TestDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: ["processConfig"],
          fields: {
            processConfig: makeField({
              type: { kind: "object", schemaRef: "ProcessConfig" },
              xml: "object",
              defaultCollapsed: true,
            }),
          },
        },
      },
      objectTypes: {
        ProcessConfig: {
          fieldOrder: ["workUnits", "taskRequirements", "prerequisiteRef"],
          fields: {
            workUnits: makeField({ type: { kind: "integer" }, xml: "element" }),
            taskRequirements: makeField({
              type: { kind: "list" },
              xml: "listOfLi",
              items: { kind: "object", schemaRef: "TaskRequirement" },
            }),
            prerequisiteRef: makeField({
              type: { kind: "defReference" },
              xml: "element",
              reference: {
                defType: "ProjectDef",
                allowAbstract: false,
                scope: "allSources",
              },
            }),
          },
        },
        TaskRequirement: {
          fieldOrder: ["taskType", "minLevel"],
          fields: {
            taskType: makeField({
              type: { kind: "defReference" },
              xml: "element",
              reference: {
                defType: "TaskDef",
                allowAbstract: false,
                scope: "allSources",
              },
            }),
            minLevel: makeField({ type: { kind: "integer" }, xml: "element" }),
          },
        },
      },
    };
  }

  it("processConfig.workUnits expands as editable number field", () => {
    const catalog = makeCollapsedSectionCatalog();
    const schema = catalog.defTypes["TestDef"];
    const def = makeDef([]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "processConfig.workUnits");
    expect(d).toBeDefined();
    expect(d!.control).toBe("number");
    expect(d!.readonly).toBe(false);
    expect(d!.fieldPath).toEqual(["processConfig", "workUnits"]);
  });

  it("processConfig.prerequisiteRef expands as editable reference field", () => {
    const catalog = makeCollapsedSectionCatalog();
    const schema = catalog.defTypes["TestDef"];
    const def = makeDef([]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "processConfig.prerequisiteRef");
    expect(d).toBeDefined();
    expect(d!.control).toBe("reference");
    expect(d!.readonly).toBe(false);
    expect(d!.reference?.defType).toBe("ProjectDef");
  });

  it("processConfig.taskRequirements expands as editable objectList when schemaRef present", () => {
    const catalog = makeCollapsedSectionCatalog();
    const schema = catalog.defTypes["TestDef"];
    const def = makeDef([]);
    const models = buildFormFieldModels(def, schema, catalog);
    const m = models.find((x) => x.key === "processConfig.taskRequirements");
    expect(m).toBeDefined();
    expect(m!.control).toBe("objectList");
    expect(m!.readonly).toBe(false);
  });

  it("processConfig section carries defaultCollapsed: true in sectionDefaults", () => {
    const catalog = makeCollapsedSectionCatalog();
    const schema = catalog.defTypes["TestDef"];
    const def = makeDef([]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "processConfig.workUnits");
    expect(d).toBeDefined();
    expect(d!.sectionDefaults[0]?.defaultCollapsed).toBe(true);
  });
});

describe("buildFormDescriptors – two sibling collapsed sections expand independently", () => {
  function makeTwoSectionCatalog(): SchemaCatalog {
    return {
      formatVersion: 1,
      packs: [],
      defTypes: {
        TestDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: ["sectionA", "sectionB"],
          fields: {
            sectionA: makeField({
              type: { kind: "object", schemaRef: "StructureProps" },
              xml: "object",
              defaultCollapsed: true,
            }),
            sectionB: makeField({
              type: { kind: "object", schemaRef: "CreatureProps" },
              xml: "object",
              defaultCollapsed: true,
            }),
          },
        },
      },
      objectTypes: {
        StructureProps: {
          fieldOrder: ["isPrimary", "isConduit", "linkedRef"],
          fields: {
            isPrimary: makeField({ type: { kind: "boolean" }, xml: "element" }),
            isConduit: makeField({ type: { kind: "boolean" }, xml: "element" }),
            linkedRef: makeField({
              type: { kind: "defReference" },
              xml: "element",
              reference: {
                defType: "EntityDef",
                allowAbstract: false,
                scope: "allSources",
              },
            }),
          },
        },
        CreatureProps: {
          fieldOrder: ["hasDivision", "levelOfSentience"],
          fields: {
            hasDivision: makeField({
              type: { kind: "boolean" },
              xml: "element",
            }),
            levelOfSentience: makeField({
              type: { kind: "enum" },
              xml: "element",
              validationHints: { allowedValues: ["None", "Basic", "Advanced"] },
            }),
          },
        },
      },
    };
  }

  it("sectionA.isPrimary expands as editable checkbox field", () => {
    const catalog = makeTwoSectionCatalog();
    const schema = catalog.defTypes["TestDef"];
    const def = makeDef([]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "sectionA.isPrimary");
    expect(d).toBeDefined();
    expect(d!.control).toBe("checkbox");
    expect(d!.readonly).toBe(false);
    expect(d!.fieldPath).toEqual(["sectionA", "isPrimary"]);
  });

  it("sectionA.linkedRef expands as editable reference field", () => {
    const catalog = makeTwoSectionCatalog();
    const schema = catalog.defTypes["TestDef"];
    const def = makeDef([]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "sectionA.linkedRef");
    expect(d).toBeDefined();
    expect(d!.control).toBe("reference");
    expect(d!.readonly).toBe(false);
    expect(d!.reference?.defType).toBe("EntityDef");
  });

  it("sectionB.levelOfSentience expands as editable select field", () => {
    const catalog = makeTwoSectionCatalog();
    const schema = catalog.defTypes["TestDef"];
    const def = makeDef([]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const d = descs.find((x) => x.key === "sectionB.levelOfSentience");
    expect(d).toBeDefined();
    expect(d!.control).toBe("select");
    expect(d!.readonly).toBe(false);
    expect(d!.allowedValues).toContain("Advanced");
  });

  it("both sections carry defaultCollapsed: true in sectionDefaults", () => {
    const catalog = makeTwoSectionCatalog();
    const schema = catalog.defTypes["TestDef"];
    const def = makeDef([]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const sectionAField = descs.find((x) => x.key === "sectionA.isPrimary");
    expect(sectionAField!.sectionDefaults[0]?.defaultCollapsed).toBe(true);
    const sectionBField = descs.find((x) => x.key === "sectionB.hasDivision");
    expect(sectionBField!.sectionDefaults[0]?.defaultCollapsed).toBe(true);
  });
});

// --- typedReferenceList ---

describe("buildFormDescriptors – empty element after Preview Save", () => {
  it("string field with textValue null returns '' not '(element)'", () => {
    const catalog = makeCatalog("ThingDef", {
      defName: makeField({ type: { kind: "string" }, xml: "element" }),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeChild("defName", null, "element")]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.control).toBe("text");
    expect(desc.value).toBe("");
    expect(desc.value).not.toBe("(element)");
    expect(desc.readonly).toBe(false);
  });

  it("description textarea with empty element returns '' not '(element)'", () => {
    const catalog = makeCatalog("ThingDef", {
      description: makeField({ type: { kind: "string" }, xml: "element" }),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeChild("description", null, "element")]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.control).toBe("textarea");
    expect(desc.value).toBe("");
    expect(desc.value).not.toBe("(element)");
    expect(desc.readonly).toBe(false);
  });

  it("defReference field with empty element returns '' not '(element)'", () => {
    const catalog = makeCatalog("ThingDef", {
      workSpeedStat: makeField({
        type: { kind: "defReference" },
        xml: "element",
        reference: {
          defType: "StatDef",
          allowAbstract: false,
          scope: "allSources",
        },
      }),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeChild("workSpeedStat", null, "element")]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.control).toBe("reference");
    expect(desc.value).toBe("");
    expect(desc.value).not.toBe("(element)");
    expect(desc.readonly).toBe(false);
  });

  it("scalar-schema field whose XML parsed as listOfLi still shows structured placeholder", () => {
    const catalog = makeCatalog("ThingDef", {
      description: makeField({ type: { kind: "string" }, xml: "element" }),
    });
    const schema = catalog.defTypes["ThingDef"];
    // Backend parsed <description><li>foo</li></description> as listOfLi, not element
    const def = makeDef([makeChild("description", null, "listOfLi")]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.control).toBe("textarea");
    expect(desc.value).not.toBe("");
    expect(typeof desc.value).toBe("string");
    expect(desc.value as string).toMatch(/listOfLi/);
  });

  it("readonlyUnknown child with null textValue still shows placeholder", () => {
    const catalog = makeCatalog("ThingDef", {});
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeChild("unknownStructured", null, "element")]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.control).toBe("readonlyUnknown");
    expect(desc.readonly).toBe(true);
    expect(desc.value).toBe("(element)");
  });
});

describe("typedReferenceList form descriptors", () => {
  function makeTypedRefCatalog(): SchemaCatalog {
    return makeCatalog("ThingDef", {
      descriptionHyperlinks: makeField({
        type: { kind: "list" },
        xml: "typedReferenceList",
        typedReference: { allowAbstract: false, scope: "allSources" },
      }),
    });
  }

  function makeTypedRefChild(nestedChildren: XmlNestedChildView[]) {
    return {
      nodeId: 20,
      name: "descriptionHyperlinks",
      textValue: null as string | null,
      listItems: [] as string[],
      xmlShape: "object" as const,
      children: nestedChildren,
      order: 0,
      known: false,
      line: null,
      column: null,
    };
  }

  it("maps descriptionHyperlinks to typedReferenceList control", () => {
    const catalog = makeTypedRefCatalog();
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeTypedRefChild([])]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.control).toBe("typedReferenceList");
    expect(desc.readonly).toBe(false);
  });

  it("extracts items from typed reference child elements", () => {
    const catalog = makeTypedRefCatalog();
    const schema = catalog.defTypes["ThingDef"];
    const children: XmlNestedChildView[] = [
      {
        nodeId: 101,
        name: "ThingDef",
        textValue: "SimpleProstheticLeg",
        listItems: [],
        xmlShape: "element",
        order: 0,
        line: null,
        column: null,
      },
      {
        nodeId: 102,
        name: "HediffDef",
        textValue: "SimpleProstheticLeg",
        listItems: [],
        xmlShape: "element",
        order: 1,
        line: null,
        column: null,
      },
    ];
    const def = makeDef([makeTypedRefChild(children)]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.value).toEqual({
      kind: "typedReferenceList",
      items: [
        { nodeId: 101, defType: "ThingDef", defName: "SimpleProstheticLeg" },
        { nodeId: 102, defType: "HediffDef", defName: "SimpleProstheticLeg" },
      ],
    });
  });

  it("preserves repeated element names in items", () => {
    const catalog = makeTypedRefCatalog();
    const schema = catalog.defTypes["ThingDef"];
    const children: XmlNestedChildView[] = [
      {
        nodeId: 201,
        name: "ThingDef",
        textValue: "Steel",
        listItems: [],
        xmlShape: "element",
        order: 0,
        line: null,
        column: null,
      },
      {
        nodeId: 202,
        name: "ThingDef",
        textValue: "Wood",
        listItems: [],
        xmlShape: "element",
        order: 1,
        line: null,
        column: null,
      },
    ];
    const def = makeDef([makeTypedRefChild(children)]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    const value = desc.value as {
      kind: string;
      items: { defType: string; defName: string }[];
    };
    expect(value.items).toHaveLength(2);
    expect(value.items[0]).toEqual(
      expect.objectContaining({ defType: "ThingDef", defName: "Steel" }),
    );
    expect(value.items[1]).toEqual(
      expect.objectContaining({ defType: "ThingDef", defName: "Wood" }),
    );
  });

  it("produces empty items when container has no children", () => {
    const catalog = makeTypedRefCatalog();
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeTypedRefChild([])]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.value).toEqual({ kind: "typedReferenceList", items: [] });
  });

  it("produces empty items when container is absent", () => {
    const catalog = makeTypedRefCatalog();
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.control).toBe("typedReferenceList");
    expect(desc.value).toEqual({ kind: "typedReferenceList", items: [] });
  });

  it("passes typedReference metadata to descriptor", () => {
    const catalog = makeTypedRefCatalog();
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeTypedRefChild([])]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.typedReference).toEqual({
      allowAbstract: false,
      scope: "allSources",
    });
  });

  it("buildFormFieldModels assigns typedReferenceList path", () => {
    const catalog = makeTypedRefCatalog();
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeTypedRefChild([])]);
    const [model] = buildFormFieldModels(def, schema, catalog);
    expect(model.control).toBe("typedReferenceList");
    expect(model.path).toEqual({
      kind: "typedReferenceList",
      objectPath: [],
      fieldName: "descriptionHyperlinks",
    });
  });
});

// --- Issue 1: XML alias element-name preservation ---

describe("buildFormDescriptors – xmlAliases element name preservation", () => {
  it("childElement path uses alias name when top-level field was matched via xmlAliases", () => {
    const catalog = makeCatalog("ThingDef", {
      sustainStartSound: makeField({
        type: { kind: "string" },
        xml: "element",
        xmlAliases: ["sustainerStartSound"],
      }),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeChild("sustainerStartSound", "SomeSound")]);
    const [model] = buildFormFieldModels(def, schema, catalog);
    expect(model.key).toBe("sustainStartSound"); // canonical key unchanged
    expect(model.path).toEqual({
      kind: "childElement",
      childName: "sustainerStartSound",
    });
  });

  it("canonical childElement path when field uses canonical name (no alias active)", () => {
    const catalog = makeCatalog("ThingDef", {
      sustainStartSound: makeField({
        type: { kind: "string" },
        xml: "element",
        xmlAliases: ["sustainerStartSound"],
      }),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeChild("sustainStartSound", "SomeSound")]);
    const [model] = buildFormFieldModels(def, schema, catalog);
    expect(model.path).toEqual({
      kind: "childElement",
      childName: "sustainStartSound",
    });
  });

  it("nestedObjectField path uses alias leaf name when nested field matched via xmlAliases", () => {
    const catalog = makeCatalogWithObjectType(
      {
        container: makeField({
          type: { kind: "object", schemaRef: "Container" },
          xml: "object",
        }),
      },
      "Container",
      {
        sustainIntervalRange: makeField({
          type: { kind: "floatRange" },
          xml: "element",
          xmlAliases: ["sustainInterval"],
        }),
      },
    );
    const schema = catalog.defTypes["ThingDef"];
    const containerChild = makeChildWithNested("container", [
      makeNestedChild("sustainInterval", "1~2"),
    ]);
    const def = makeDef([containerChild]);
    const models = buildFormFieldModels(def, schema, catalog);
    const m = models.find((x) => x.key === "container.sustainIntervalRange");
    expect(m).toBeDefined();
    expect(m!.path).toEqual({
      kind: "nestedObjectField",
      objectPath: ["container"],
      fieldName: "sustainInterval",
    });
  });
});

// --- Issue 2: Object-list nested fields stored as readonly in ObjectListItemValue ---

function makeListItemView(
  children: XmlNestedChildView[],
  attrs: XmlAttributeView[] = [],
): XmlListItemView {
  return {
    nodeId: 100,
    textValue: null,
    attributes: attrs,
    children,
    order: 0,
    line: null,
    column: null,
    selfClosing: false,
  };
}

function makeNestedListChild(
  name: string,
  liItemCount: number,
): XmlNestedChildView {
  const liItems: XmlListItemView[] = Array.from(
    { length: liItemCount },
    (_, i) => ({
      nodeId: 200 + i,
      textValue: null,
      attributes: [],
      children: [],
      order: i,
      line: null,
      column: null,
      selfClosing: false,
    }),
  );
  return {
    nodeId: 150,
    name,
    textValue: null,
    listItems: [],
    xmlShape: "listOfLi",
    order: 0,
    line: null,
    column: null,
    liItems,
  };
}

describe("buildFormDescriptors – object-list item nested list fields rendered recursively", () => {
  function makeNestedListItemCatalog(): SchemaCatalog {
    return {
      formatVersion: 1,
      packs: [],
      defTypes: {
        TestDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: ["audioClips"],
          fields: {
            audioClips: makeField({
              type: { kind: "list" },
              xml: "listOfLi",
              items: { kind: "object", schemaRef: "AudioClip" },
            }),
          },
        },
      },
      objectTypes: {
        AudioClip: {
          fieldOrder: ["clipName", "grainSources"],
          fields: {
            clipName: makeField({ type: { kind: "string" }, xml: "element" }),
            grainSources: makeField({
              type: { kind: "list" },
              xml: "listOfLi",
              items: { kind: "object", schemaRef: "GrainSource" },
            }),
          },
        },
        GrainSource: {
          fieldOrder: [],
          fields: {},
        },
      },
    };
  }

  it("stores listOfLi object fields in ObjectListItemValue.fields as objectList", () => {
    const catalog = makeNestedListItemCatalog();
    const schema = catalog.defTypes["TestDef"];

    const audioClipsChild: DefEditorView["children"][number] = {
      nodeId: 5,
      name: "audioClips",
      textValue: null,
      listItems: [],
      xmlShape: "listOfLi",
      order: 0,
      known: true,
      line: null,
      column: null,
      liItems: [makeListItemView([makeNestedListChild("grainSources", 3)])],
    };
    const def = makeDef([audioClipsChild]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const audioClipsDesc = descs.find((x) => x.key === "audioClips");
    expect(audioClipsDesc).toBeDefined();
    const value = audioClipsDesc!.value as {
      kind: "objectList";
      items: ObjectListItemValue[];
    };
    expect(value.items).toHaveLength(1);
    const grainSourcesField = value.items[0].fields["grainSources"];
    expect(grainSourcesField).toBeDefined();
    expect(grainSourcesField!.kind).toBe("objectList");
    const grainSourcesList = grainSourcesField as {
      kind: "objectList";
      itemSchemaRef: string;
      items: ObjectListItemValue[];
    };
    expect(grainSourcesList.itemSchemaRef).toBe("GrainSource");
    expect(grainSourcesList.items).toHaveLength(3);
  });

  it("nested object-list field item count matches liItems in the view", () => {
    const catalog = makeNestedListItemCatalog();
    const schema = catalog.defTypes["TestDef"];
    const audioClipsChild: DefEditorView["children"][number] = {
      nodeId: 5,
      name: "audioClips",
      textValue: null,
      listItems: [],
      xmlShape: "listOfLi",
      order: 0,
      known: true,
      line: null,
      column: null,
      liItems: [makeListItemView([makeNestedListChild("grainSources", 2)])],
    };
    const def = makeDef([audioClipsChild]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const audioClipsDesc = descs.find((x) => x.key === "audioClips");
    const value = audioClipsDesc!.value as {
      kind: "objectList";
      items: ObjectListItemValue[];
    };
    const grainSourcesField = value.items[0].fields["grainSources"] as {
      kind: "objectList";
      items: ObjectListItemValue[];
    };
    expect(grainSourcesField.kind).toBe("objectList");
    expect(grainSourcesField.items).toHaveLength(2);
  });

  it("editable scalar fields inside ObjectListItemValue are still stored as scalars", () => {
    const catalog = makeNestedListItemCatalog();
    const schema = catalog.defTypes["TestDef"];
    const audioClipsChild: DefEditorView["children"][number] = {
      nodeId: 5,
      name: "audioClips",
      textValue: null,
      listItems: [],
      xmlShape: "listOfLi",
      order: 0,
      known: true,
      line: null,
      column: null,
      liItems: [
        makeListItemView([
          makeNestedChild("clipName", "MyClip"),
          makeNestedListChild("grainSources", 1),
        ]),
      ],
    };
    const def = makeDef([audioClipsChild]);
    const descs = buildFormDescriptors(def, schema, catalog);
    const value = descs.find((x) => x.key === "audioClips")!.value as {
      kind: "objectList";
      items: ObjectListItemValue[];
    };
    const clipNameField = value.items[0].fields["clipName"];
    expect(clipNameField).toBeDefined();
    expect(clipNameField!.kind).toBe("scalar");
    expect((clipNameField as { kind: "scalar"; value: string }).value).toBe(
      "MyClip",
    );
  });
});

// --- Issue 3: Discriminator resolution for single-object element fields ---

describe("buildFormDescriptors – discriminator resolution for single-object elements", () => {
  function makeDiscriminatedObjectCatalog(): SchemaCatalog {
    return {
      formatVersion: 1,
      packs: [],
      defTypes: {
        TestDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: ["container"],
          fields: {
            container: makeField({
              type: { kind: "object", schemaRef: "ParamMapping" },
              xml: "object",
            }),
          },
        },
      },
      objectTypes: {
        ParamMapping: {
          fieldOrder: ["inputParam"],
          fields: {
            inputParam: makeField({
              type: { kind: "object", schemaRef: "ParamSource" },
              xml: "object",
            }),
          },
        },
        ParamSource: {
          fieldOrder: ["Class"],
          fields: {
            Class: makeField({ type: { kind: "typeName" }, xml: "attribute" }),
          },
          discriminator: {
            attribute: "Class",
            fallbackSchemaRef: "ParamSource",
            allowMissing: false,
            allowUnknown: true,
            variants: {
              ParamSource_External: "ParamSource_External",
            },
          },
        },
        ParamSource_External: {
          fieldOrder: ["Class", "paramKey", "defaultValue"],
          inherits: ["ParamSource"],
          fields: {
            paramKey: makeField({ type: { kind: "string" }, xml: "element" }),
            defaultValue: makeField({
              type: { kind: "float" },
              xml: "element",
            }),
          },
        },
      },
    };
  }

  it("resolves variant schema when nested object element has Class attribute", () => {
    const catalog = makeDiscriminatedObjectCatalog();
    const schema = catalog.defTypes["TestDef"];

    const inputParamChild: XmlNestedChildView = {
      nodeId: 20,
      name: "inputParam",
      textValue: null,
      listItems: [],
      xmlShape: "object",
      attributes: [
        { name: "Class", value: "ParamSource_External", known: true },
      ],
      children: [makeNestedChild("paramKey", "myParam")],
      order: 0,
      line: null,
      column: null,
    };

    const def = makeDef([makeChildWithNested("container", [inputParamChild])]);
    const descs = buildFormDescriptors(def, schema, catalog);

    const paramKeyDesc = descs.find(
      (x) => x.key === "container.inputParam.paramKey",
    );
    expect(paramKeyDesc).toBeDefined();
    expect(paramKeyDesc!.control).toBe("text");
    expect(paramKeyDesc!.value).toBe("myParam");
    expect(paramKeyDesc!.readonly).toBe(false);
  });

  it("falls back to base schema fields when Class attribute is absent and allowMissing is handled", () => {
    const catalog = makeDiscriminatedObjectCatalog();
    const schema = catalog.defTypes["TestDef"];

    // inputParam element with no Class attribute - discriminator.allowMissing is false,
    // so resolveObjectSchema returns null. Base schema (ParamSource) has only the
    // Class attribute field, so no element-level descriptors are produced.
    const inputParamChild: XmlNestedChildView = {
      nodeId: 20,
      name: "inputParam",
      textValue: null,
      listItems: [],
      xmlShape: "object",
      children: [],
      order: 0,
      line: null,
      column: null,
    };

    const def = makeDef([makeChildWithNested("container", [inputParamChild])]);
    const descs = buildFormDescriptors(def, schema, catalog);

    // The variant-specific paramKey should NOT appear without a Class attribute
    expect(
      descs.find((x) => x.key === "container.inputParam.paramKey"),
    ).toBeUndefined();
  });
});

describe("nested attribute fields (nestedAttribute path)", () => {
  function makeObjectChildWithAttr(
    name: string,
    nodeId: number,
    attrName: string,
    attrValue: string,
    children: XmlNestedChildView[] = [],
  ) {
    return {
      nodeId,
      name,
      textValue: null as string | null,
      listItems: [] as string[],
      xmlShape: "object" as const,
      children,
      attributes: [{ name: attrName, value: attrValue, known: true }],
      order: 0,
      known: false,
      line: null,
      column: null,
    };
  }

  it("builds editable nestedAttribute descriptor for attribute field on nested object", () => {
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {
        QuestNode: {
          fieldOrder: ["Class", "selectionWeight"],
          fields: {
            Class: makeField({ type: { kind: "typeName" }, xml: "attribute" }),
            selectionWeight: makeField({
              type: { kind: "string" },
              xml: "element",
            }),
          },
        },
      },
      defTypes: {
        QuestScriptDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: ["root"],
          fields: {
            root: makeField({
              type: { kind: "object", schemaRef: "QuestNode" },
              xml: "object",
            }),
          },
        },
      },
    };
    const schema = catalog.defTypes["QuestScriptDef"];
    const rootChild = makeObjectChildWithAttr(
      "root",
      5,
      "Class",
      "QuestNode_Sequence",
    );
    const def: DefEditorView = {
      nodeId: 1,
      defType: "QuestScriptDef",
      defName: "SomeQuest",
      label: null,
      parentName: null,
      line: null,
      column: null,
      attributes: [],
      children: [rootChild],
    };
    const models = buildFormFieldModels(def, schema, catalog);
    const classModel = models.find((m) => m.key === "root.Class");
    expect(classModel).toBeDefined();
    expect(classModel!.readonly).toBe(false);
    expect(classModel!.path.kind).toBe("nestedAttribute");
    if (classModel!.path.kind === "nestedAttribute") {
      expect(classModel!.path.objectPath).toEqual(["root"]);
      expect(classModel!.path.attributeName).toBe("Class");
    }
    expect(classModel!.sourceNodeId).toBe(5);
  });

  it("resolves discriminator at top-level and shows variant fields alongside base attribute", () => {
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {
        QuestNode: {
          fieldOrder: ["Class", "selectionWeight"],
          fields: {
            Class: makeField({ type: { kind: "typeName" }, xml: "attribute" }),
            selectionWeight: makeField({
              type: { kind: "string" },
              xml: "element",
            }),
          },
          discriminator: {
            attribute: "Class",
            fallbackSchemaRef: "QuestNode",
            allowMissing: true,
            allowUnknown: true,
            variants: { QuestNode_Sequence: "QuestNode_Sequence" },
          },
        },
        QuestNode_Sequence: {
          fieldOrder: ["nodes"],
          fields: {
            nodes: makeField({ type: { kind: "list" }, xml: "listOfLi" }),
          },
          inherits: ["QuestNode"],
        },
      },
      defTypes: {
        QuestScriptDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: ["root"],
          fields: {
            root: makeField({
              type: { kind: "object", schemaRef: "QuestNode" },
              xml: "object",
            }),
          },
        },
      },
    };
    const schema = catalog.defTypes["QuestScriptDef"];
    const rootChild = makeObjectChildWithAttr(
      "root",
      5,
      "Class",
      "QuestNode_Sequence",
    );
    const def: DefEditorView = {
      nodeId: 1,
      defType: "QuestScriptDef",
      defName: "SomeQuest",
      label: null,
      parentName: null,
      line: null,
      column: null,
      attributes: [],
      children: [rootChild],
    };
    const models = buildFormFieldModels(def, schema, catalog);
    const classModel = models.find((m) => m.key === "root.Class");
    const weightModel = models.find((m) => m.key === "root.selectionWeight");
    const nodesModel = models.find((m) => m.key === "root.nodes");
    expect(classModel).toBeDefined();
    expect(classModel!.path.kind).toBe("nestedAttribute");
    expect(classModel!.readonly).toBe(false);
    expect(weightModel).toBeDefined();
    expect(weightModel!.readonly).toBe(false);
    expect(nodesModel).toBeDefined();
  });

  it("unknown discriminator class still exposes base attribute field as editable", () => {
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {
        QuestNode: {
          fieldOrder: ["Class", "selectionWeight"],
          fields: {
            Class: makeField({ type: { kind: "typeName" }, xml: "attribute" }),
            selectionWeight: makeField({
              type: { kind: "string" },
              xml: "element",
            }),
          },
          discriminator: {
            attribute: "Class",
            fallbackSchemaRef: "QuestNode",
            allowMissing: true,
            allowUnknown: true,
            variants: {},
          },
        },
      },
      defTypes: {
        QuestScriptDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: ["root"],
          fields: {
            root: makeField({
              type: { kind: "object", schemaRef: "QuestNode" },
              xml: "object",
            }),
          },
        },
      },
    };
    const schema = catalog.defTypes["QuestScriptDef"];
    const rootChild = makeObjectChildWithAttr(
      "root",
      5,
      "Class",
      "QuestNode_Custom",
    );
    const def: DefEditorView = {
      nodeId: 1,
      defType: "QuestScriptDef",
      defName: "SomeQuest",
      label: null,
      parentName: null,
      line: null,
      column: null,
      attributes: [],
      children: [rootChild],
    };
    const models = buildFormFieldModels(def, schema, catalog);
    const classModel = models.find((m) => m.key === "root.Class");
    expect(classModel).toBeDefined();
    expect(classModel!.readonly).toBe(false);
    expect(classModel!.path.kind).toBe("nestedAttribute");
  });

  it("maps color field to color control", () => {
    const catalog = makeCatalog("ColorDef", {
      color: makeField({ type: { kind: "color" } }),
    });
    const schema = catalog.defTypes["ColorDef"];
    const def = makeDef([makeChild("color", "(118, 49, 57)")]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.control).toBe("color");
    expect(desc.value).toBe("(118, 49, 57)");
  });
});

// ---------------------------------------------------------------------------
// keyedObjectMap descriptor and model tests
// ---------------------------------------------------------------------------

function makeKeyedObjectMapCatalog(): SchemaCatalog {
  return {
    formatVersion: 1,
    packs: [],
    defTypes: {
      TestDef: {
        inherits: [],
        abstractType: false,
        fieldOrder: ["keyedMap"],
        fields: {
          keyedMap: makeField({
            type: { kind: "list" },
            xml: "keyedObjectMap",
            items: { kind: "object", schemaRef: "PartValue" },
            keyReference: { defType: "TagDef", allowAbstract: false, scope: "allSources" },
          }),
        },
      },
    },
    objectTypes: {
      PartValue: {
        fieldOrder: ["knownField", "count"],
        fields: {
          knownField: makeField({ type: { kind: "string" }, xml: "element" }),
          count: makeField({ type: { kind: "integer" }, xml: "element" }),
        },
      },
    },
  };
}

function makeKeyedMapLiItem(keyText: string, valueChildren: XmlNestedChildView[]): XmlListItemView {
  return {
    nodeId: 100,
    textValue: null,
    attributes: [],
    children: [
      { nodeId: 101, name: "key", textValue: keyText, listItems: [], xmlShape: "element", order: 0, line: null, column: null },
      { nodeId: 102, name: "value", textValue: null, listItems: [], xmlShape: "object", children: valueChildren, order: 1, line: null, column: null },
    ],
    order: 0,
    line: null,
    column: null,
    selfClosing: false,
  };
}

describe("buildFormDescriptors – keyedObjectMap", () => {
  it("returns objectList control for a keyedObjectMap field with schemaRef", () => {
    const catalog = makeKeyedObjectMapCatalog();
    const schema = catalog.defTypes["TestDef"];
    const def = makeDef([]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.control).toBe("objectList");
    expect(desc.xmlShape).toBe("keyedObjectMap");
    expect(desc.readonly).toBe(false);
  });

  it("populates items from li children using <key> and <value> structure", () => {
    const catalog = makeKeyedObjectMapCatalog();
    const schema = catalog.defTypes["TestDef"];
    const liItem = makeKeyedMapLiItem("Root", [
      { nodeId: 200, name: "knownField", textValue: "hello", listItems: [], xmlShape: "element", order: 0, line: null, column: null },
    ]);
    const child: DefEditorView["children"][number] = {
      nodeId: 50,
      name: "keyedMap",
      textValue: null,
      listItems: [],
      xmlShape: "listOfLi",
      order: 0,
      known: true,
      line: null,
      column: null,
      liItems: [liItem],
    };
    const def = makeDef([child]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    const val = desc.value as { kind: string; items: { className: string }[] };
    expect(val.kind).toBe("objectList");
    expect(val.items).toHaveLength(1);
    expect(val.items[0].className).toBe("Root");
  });

  it("sets itemSchemaRef on the descriptor", () => {
    const catalog = makeKeyedObjectMapCatalog();
    const schema = catalog.defTypes["TestDef"];
    const def = makeDef([]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.itemSchemaRef).toBe("PartValue");
  });

  it("forwards keyReference metadata on the descriptor", () => {
    const catalog = makeKeyedObjectMapCatalog();
    const schema = catalog.defTypes["TestDef"];
    const def = makeDef([]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    expect(desc.keyReference?.defType).toBe("TagDef");
  });

  it("counts unknown value-object children in initialUnknownFieldCount", () => {
    const catalog = makeKeyedObjectMapCatalog();
    const schema = catalog.defTypes["TestDef"];
    const liItem = makeKeyedMapLiItem("Root", [
      { nodeId: 200, name: "knownField", textValue: "hello", listItems: [], xmlShape: "element", order: 0, line: null, column: null },
      { nodeId: 201, name: "surprisingField", textValue: "?", listItems: [], xmlShape: "element", order: 1, line: null, column: null },
    ]);
    const child: DefEditorView["children"][number] = {
      nodeId: 50,
      name: "keyedMap",
      textValue: null,
      listItems: [],
      xmlShape: "listOfLi",
      order: 0,
      known: true,
      line: null,
      column: null,
      liItems: [liItem],
    };
    const def = makeDef([child]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    const val = desc.value as { kind: string; items: { initialUnknownFieldCount: number }[] };
    expect(val.items[0].initialUnknownFieldCount).toBe(1);
  });

  it("assigns objectList path kind to the form field model", () => {
    const catalog = makeKeyedObjectMapCatalog();
    const schema = catalog.defTypes["TestDef"];
    const def = makeDef([]);
    const [model] = buildFormFieldModels(def, schema, catalog);
    expect(model.path.kind).toBe("objectList");
    expect(model.xmlShape).toBe("keyedObjectMap");
    expect(model.readonly).toBe(false);
  });

  it("does not use AnimationDef, keyframeParts, or PawnRenderNodeTagDef names", () => {
    // Verifies the builder is generic -- only synthetic type names appear.
    const catalog = makeKeyedObjectMapCatalog();
    const schema = catalog.defTypes["TestDef"];
    const def = makeDef([]);
    const [desc] = buildFormDescriptors(def, schema, catalog);
    const json = JSON.stringify(desc);
    expect(json).not.toContain("AnimationDef");
    expect(json).not.toContain("keyframeParts");
    expect(json).not.toContain("PawnRenderNodeTagDef");
  });
});

// --- Issue 05: Form View top-level visibility filtering ---
//
// `visibleTopLevelFieldIds` is an optional filter applied before expensive nested
// expansion. `undefined`/`null` (or the argument omitted entirely) must reproduce today's
// full-form behavior exactly; a Set filters by the CANONICAL top-level Def schema field key
// (the same key `getAllSchemaFields`/`allFields` already uses, and exactly what becomes
// `descriptor.fieldPath[0]`), never by an XML alias.

describe("buildFormDescriptors – Form View visibility filtering (issue 05)", () => {
  it("omitting the argument, passing undefined, and passing null are all identical to today's unfiltered form", () => {
    const catalog = makeCatalog("ThingDef", {
      defName: makeField(),
      description: makeField(),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([
      makeChild("defName", "Steel"),
      makeChild("description", "Old"),
    ]);

    const omitted = buildFormDescriptors(def, schema, catalog);
    const withUndefined = buildFormDescriptors(def, schema, catalog, undefined);
    const withNull = buildFormDescriptors(def, schema, catalog, null);

    expect(withUndefined).toEqual(omitted);
    expect(withNull).toEqual(omitted);
    expect(omitted.map((d) => d.key)).toEqual(["defName", "description"]);
  });

  it("skips a hidden scalar top-level root entirely", () => {
    const catalog = makeCatalog("ThingDef", {
      defName: makeField(),
      description: makeField(),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([
      makeChild("defName", "Steel"),
      makeChild("description", "Old"),
    ]);

    const descs = buildFormDescriptors(def, schema, catalog, new Set(["defName"]));

    expect(descs.map((d) => d.key)).toEqual(["defName"]);
  });

  it("does not resurrect a hidden known field's XML as an unknown field", () => {
    const catalog = makeCatalog("ThingDef", { description: makeField() });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeChild("description", "Old")]);

    // Hide every known field. If `description` were treated as "not handled" once hidden,
    // its XML child would incorrectly reappear as a readonlyUnknown field below.
    const descs = buildFormDescriptors(def, schema, catalog, new Set());

    expect(descs).toHaveLength(0);
  });

  it("hides a field matched only via an XML alias, keyed by its canonical schema name", () => {
    const catalog = makeCatalog("ThingDef", {
      label: makeField({ xmlAliases: ["labelOld"] }),
    });
    const schema = catalog.defTypes["ThingDef"];
    // The document uses the alias element name, not the canonical field name.
    const def = makeDef([makeChild("labelOld", "Hello")]);

    const descs = buildFormDescriptors(def, schema, catalog, new Set());

    expect(descs).toHaveLength(0);
  });

  it("keeps an unknown XML child visible regardless of the visibility filter", () => {
    const catalog = makeCatalog("ThingDef", { defName: makeField() });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([
      makeChild("defName", "Steel"),
      makeChild("modAddedThing", "mystery"),
    ]);

    // Hide every known top-level field.
    const descs = buildFormDescriptors(def, schema, catalog, new Set());

    const unknown = descs.find((d) => d.key === "modAddedThing");
    expect(unknown).toBeDefined();
    expect(unknown!.control).toBe("readonlyUnknown");
  });

  it("treats two XML elements sharing the same name as a single top-level root for visibility (Plan.md section 5/12)", () => {
    // Plan.md section 12: "Duplicate XML elements: ... Form Views neither fix nor worsen
    // duplicate XML behavior. Treat all occurrences as a single top-level root for visibility."
    // `def.children` already collapses same-named entries to one map key before the
    // visibility check runs (the existing `childByName` Map construction), so hiding/showing
    // `description` can never leave one occurrence visible and the other hidden - there is
    // only ever one descriptor for it either way, matching today's (pre-Form-Views) duplicate
    // handling exactly.
    const catalog = makeCatalog("ThingDef", { defName: makeField(), description: makeField() });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([
      makeChild("defName", "Steel"),
      makeChild("description", "First"),
      makeChild("description", "Second"),
    ]);

    const visible = buildFormDescriptors(def, schema, catalog);
    expect(visible.filter((d) => d.key === "description")).toHaveLength(1);

    const hidden = buildFormDescriptors(def, schema, catalog, new Set(["defName"]));
    expect(hidden.some((d) => d.key === "description")).toBe(false);
    expect(hidden).toHaveLength(1);
  });
});

describe("buildFormDescriptors – hidden roots skip expensive expansion, not just post-hoc filtering (issue 05)", () => {
  /**
   * A catalog whose `objectTypes[ref]` throws the moment it is *read*. Any code path that
   * resolves the object schema for `ref` - discriminator lookup, `buildNestedObjectDescriptors`,
   * or object-list item construction - throws immediately. This proves the visibility filter
   * short-circuits *before* schema resolution, not merely that hidden descriptors are absent
   * from the final array (which a slower construct-then-filter implementation would also satisfy).
   */
  function definePoisonedObjectType(catalog: SchemaCatalog, ref: string, reason: string) {
    Object.defineProperty(catalog.objectTypes, ref, {
      enumerable: true,
      configurable: true,
      get(): never {
        throw new Error(reason);
      },
    });
  }

  it("control: expanding an unfiltered object root does reach the object schema (proves the trap is live)", () => {
    const catalog = makeCatalog("ThingDef", {
      defName: makeField(),
      graphicData: makeField({
        type: { kind: "object", schemaRef: "GraphicData" },
        xml: "object",
      }),
    });
    definePoisonedObjectType(
      catalog,
      "GraphicData",
      "buildNestedObjectDescriptors reached the hidden object root",
    );
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([
      makeChild("defName", "Steel"),
      { ...makeChild("graphicData", null, "object"), children: [] },
    ]);

    expect(() => buildFormDescriptors(def, schema, catalog)).toThrow(
      /reached the hidden object root/,
    );
  });

  it("hiding an object root never resolves its schema or expands nested descriptors", () => {
    const catalog = makeCatalog("ThingDef", {
      defName: makeField(),
      graphicData: makeField({
        type: { kind: "object", schemaRef: "GraphicData" },
        xml: "object",
      }),
    });
    definePoisonedObjectType(
      catalog,
      "GraphicData",
      "buildNestedObjectDescriptors reached the hidden object root",
    );
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([
      makeChild("defName", "Steel"),
      { ...makeChild("graphicData", null, "object"), children: [] },
    ]);

    let descs: ReturnType<typeof buildFormDescriptors> = [];
    expect(() => {
      descs = buildFormDescriptors(def, schema, catalog, new Set(["defName"]));
    }).not.toThrow();
    expect(descs.map((d) => d.key)).toEqual(["defName"]);
    expect(descs.some((d) => d.fieldPath[0] === "graphicData")).toBe(false);
  });

  it("control: expanding an unfiltered editable object-list root does reach the item schema (proves the trap is live)", () => {
    const catalog = makeCatalog("ThingDef", {
      defName: makeField(),
      comps: makeField({
        type: { kind: "list" },
        xml: "listOfLi",
        items: { kind: "object", schemaRef: "CompProperties" },
      }),
    });
    definePoisonedObjectType(
      catalog,
      "CompProperties",
      "buildObjectListItemValue reached the hidden object-list root",
    );
    const schema = catalog.defTypes["ThingDef"];
    const compsChild = {
      ...makeChild("comps", null, "listOfLi"),
      liItems: [makeListItemView([])],
    };
    const def = makeDef([makeChild("defName", "Steel"), compsChild]);

    expect(() => buildFormDescriptors(def, schema, catalog)).toThrow(
      /reached the hidden object-list root/,
    );
  });

  it("hiding an editable object-list root never constructs its item values", () => {
    const catalog = makeCatalog("ThingDef", {
      defName: makeField(),
      comps: makeField({
        type: { kind: "list" },
        xml: "listOfLi",
        items: { kind: "object", schemaRef: "CompProperties" },
      }),
    });
    definePoisonedObjectType(
      catalog,
      "CompProperties",
      "buildObjectListItemValue reached the hidden object-list root",
    );
    const schema = catalog.defTypes["ThingDef"];
    const compsChild = {
      ...makeChild("comps", null, "listOfLi"),
      liItems: [makeListItemView([])],
    };
    const def = makeDef([makeChild("defName", "Steel"), compsChild]);

    let descs: ReturnType<typeof buildFormDescriptors> = [];
    expect(() => {
      descs = buildFormDescriptors(def, schema, catalog, new Set(["defName"]));
    }).not.toThrow();
    expect(descs.some((d) => d.key === "comps")).toBe(false);
  });
});

describe("buildFormFieldModels – Form View visibility filtering (issue 05)", () => {
  it("forwards the visible set through to descriptors and excludes hidden roots from models", () => {
    const catalog = makeCatalog("ThingDef", {
      defName: makeField(),
      description: makeField(),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([
      makeChild("defName", "Steel"),
      makeChild("description", "Old"),
    ]);

    const models = buildFormFieldModels(def, schema, catalog, new Set(["defName"]));

    expect(models.map((m) => m.key)).toEqual(["defName"]);
  });

  it("omitting the argument matches today's unfiltered model output", () => {
    const catalog = makeCatalog("ThingDef", {
      defName: makeField(),
      description: makeField(),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([
      makeChild("defName", "Steel"),
      makeChild("description", "Old"),
    ]);

    expect(buildFormFieldModels(def, schema, catalog, undefined)).toEqual(
      buildFormFieldModels(def, schema, catalog),
    );
  });
});

describe("collectEffectiveTopLevelDefFields (issue 06, Plan.md section 5)", () => {
  it("collects direct fields for a Def type with no inheritance", () => {
    const catalog = makeCatalog("ThingDef", {
      defName: makeField(),
      description: makeField(),
    });
    const known = collectEffectiveTopLevelDefFields(catalog.defTypes["ThingDef"], catalog);
    expect([...known].sort()).toEqual(["defName", "description"]);
  });

  it("includes inherited ancestor fields alongside the concrete type's own fields", () => {
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {},
      defTypes: {
        BaseDef: {
          inherits: [],
          abstractType: true,
          fieldOrder: [],
          fields: { defName: makeField(), category: makeField() },
        },
        DerivedDef: {
          inherits: ["BaseDef"],
          abstractType: false,
          fieldOrder: [],
          fields: { damage: makeField() },
        },
      },
    };
    const known = collectEffectiveTopLevelDefFields(catalog.defTypes["DerivedDef"], catalog);
    expect([...known].sort()).toEqual(["category", "damage", "defName"]);
  });

  it("counts a field declared on both an ancestor and the concrete type only once (duplicate inherited name)", () => {
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {},
      defTypes: {
        BaseDef: {
          inherits: [],
          abstractType: true,
          fieldOrder: [],
          fields: { label: makeField({ label: "Base label" }) },
        },
        DerivedDef: {
          inherits: ["BaseDef"],
          abstractType: false,
          fieldOrder: [],
          // Same field name re-declared on the concrete type -- must appear exactly once in
          // the resulting set, matching `buildFormDescriptors`'s own single-descriptor-per-
          // canonical-field-name behavior (never two "label" entries).
          fields: { label: makeField({ label: "Derived label" }) },
        },
      },
    };
    const known = collectEffectiveTopLevelDefFields(catalog.defTypes["DerivedDef"], catalog);
    expect([...known]).toEqual(["label"]);
  });

  it("matches the exact set buildFormDescriptors renders for the same Def/catalog", () => {
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {},
      defTypes: {
        BaseDef: {
          inherits: [],
          abstractType: true,
          fieldOrder: [],
          fields: { defName: makeField(), category: makeField() },
        },
        DerivedDef: {
          inherits: ["BaseDef"],
          abstractType: false,
          fieldOrder: [],
          fields: { damage: makeField() },
        },
      },
    };
    const def = makeDef([]);
    const rendered = buildFormDescriptors(def, catalog.defTypes["DerivedDef"], catalog).map(
      (d) => d.key,
    );
    const known = collectEffectiveTopLevelDefFields(catalog.defTypes["DerivedDef"], catalog);
    expect([...known].sort()).toEqual([...rendered].sort());
  });

  it("does not throw and returns only reachable fields for a cyclic inherits chain", () => {
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {},
      defTypes: {
        A: { inherits: ["B"], abstractType: true, fieldOrder: [], fields: { fieldA: makeField() } },
        B: { inherits: ["A"], abstractType: true, fieldOrder: [], fields: { fieldB: makeField() } },
      },
    };
    const known = collectEffectiveTopLevelDefFields(catalog.defTypes["A"], catalog);
    expect([...known].sort()).toEqual(["fieldA", "fieldB"]);
  });
});

describe("collectTopLevelFieldSummaries (issue 07, Plan.md section 8)", () => {
  it("summarizes a scalar field with no XML data as not having a value", () => {
    const catalog = makeCatalog("ThingDef", { defName: makeField() });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([]);
    const [summary] = collectTopLevelFieldSummaries(def, schema, catalog);
    expect(summary).toEqual({
      id: "defName",
      label: "defName",
      isSection: false,
      controlKind: "text",
      hasValue: false,
    });
  });

  it("reports hasValue true once the XML child carries non-empty text", () => {
    const catalog = makeCatalog("ThingDef", { defName: makeField() });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeChild("defName", "Steel")]);
    const [summary] = collectTopLevelFieldSummaries(def, schema, catalog);
    expect(summary.hasValue).toBe(true);
  });

  it("reports hasValue from the element attribute for an attribute-shaped field", () => {
    const catalog = makeCatalog("ThingDef", {
      Abstract: makeField({ xml: "attribute", type: { kind: "boolean" } }),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def: DefEditorView = {
      ...makeDef([]),
      attributes: [{ name: "Abstract", value: "true", known: true }],
    };
    const [summary] = collectTopLevelFieldSummaries(def, schema, catalog);
    expect(summary.hasValue).toBe(true);
    expect(summary.controlKind).toBe("checkbox");
  });

  it("marks an expandable object-root field as a section, regardless of XML data", () => {
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {
        GraphicData: {
          fieldOrder: [],
          fields: { texPath: makeField() },
        },
      },
      defTypes: {
        ThingDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: [],
          fields: {
            graphicData: makeField({
              xml: "object",
              type: { kind: "object", schemaRef: "GraphicData" },
            }),
          },
        },
      },
    };
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([]);
    const [summary] = collectTopLevelFieldSummaries(def, schema, catalog);
    expect(summary.isSection).toBe(true);
    expect(summary.hasValue).toBe(false);
  });

  it("resolves an XML-aliased child under its canonical schema field id (issue 07 step 6/13)", () => {
    const catalog = makeCatalog("ThingDef", {
      defName: makeField({ xmlAliases: ["Name"] }),
    });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([makeChild("Name", "Steel")]);
    const [summary] = collectTopLevelFieldSummaries(def, schema, catalog);
    expect(summary.id).toBe("defName");
    expect(summary.hasValue).toBe(true);
  });

  it("preserves the exact same schema order as collectEffectiveTopLevelDefFields", () => {
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {},
      defTypes: {
        BaseDef: {
          inherits: [],
          abstractType: true,
          fieldOrder: [],
          fields: { defName: makeField(), category: makeField() },
        },
        DerivedDef: {
          inherits: ["BaseDef"],
          abstractType: false,
          fieldOrder: [],
          fields: { damage: makeField() },
        },
      },
    };
    const schema = catalog.defTypes["DerivedDef"];
    const def = makeDef([]);
    const summaries = collectTopLevelFieldSummaries(def, schema, catalog);
    const known = [...collectEffectiveTopLevelDefFields(schema, catalog)];
    expect(summaries.map((s) => s.id)).toEqual(known);
  });

  it("never lists an orphaned field id that is no longer part of the current schema", () => {
    const catalog = makeCatalog("ThingDef", { defName: makeField() });
    const schema = catalog.defTypes["ThingDef"];
    const def = makeDef([]);
    const summaries = collectTopLevelFieldSummaries(def, schema, catalog);
    expect(summaries.map((s) => s.id)).not.toContain("noLongerAField");
  });
});
