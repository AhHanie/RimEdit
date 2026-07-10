import { describe, expect, it } from "vitest";
import type { FieldSchema, SchemaCatalog } from "../../schema-catalog";
import type { XmlChildView } from "../../xml-editor";
import {
  emptyFieldValue,
  fieldValueToInitialElement,
  isStructurallySupportedField,
  parsedViewsToFieldValue,
} from "./patchValueShape";

const catalog: SchemaCatalog = {
  formatVersion: 1,
  packs: [],
  defTypes: {
    Def: {
      inherits: [],
      abstractType: true,
      fieldOrder: ["modExtensions"],
      fields: {
        modExtensions: {
          type: { kind: "list" },
          required: false,
          examples: [],
          repeatable: false,
          xml: "listOfLi",
          flags: false,
        },
      },
    },
    ThingDef: {
      inherits: ["Def"],
      abstractType: false,
      fieldOrder: ["label", "tags", "comps"],
      fields: {
        label: {
          type: { kind: "string" },
          required: false,
          examples: [],
          repeatable: false,
          xml: "element",
          flags: false,
        },
        tags: {
          type: { kind: "list" },
          required: false,
          examples: [],
          repeatable: false,
          xml: "listOfLi",
          flags: false,
        },
        comps: {
          type: { kind: "list" },
          required: false,
          examples: [],
          repeatable: false,
          xml: "listOfLi",
          flags: false,
          items: { kind: "object", schemaRef: "CompProperties" },
        },
        graphicData: {
          type: { kind: "defReference" },
          required: false,
          examples: [],
          repeatable: false,
          xml: "element",
          flags: false,
        },
      },
    },
  },
  objectTypes: {
    CompProperties: {
      fieldOrder: [],
      fields: {},
      discriminator: {
        attribute: "Class",
        allowMissing: false,
        allowUnknown: true,
        variants: { CompProperties_Foo: "CompProperties_Foo" },
      },
    },
    CompProperties_Foo: {
      inherits: ["CompProperties"],
      fieldOrder: ["hitPoints"],
      fields: {
        hitPoints: {
          type: { kind: "integer" },
          required: false,
          examples: [],
          repeatable: false,
          xml: "element",
          flags: false,
        },
      },
    },
  },
};

const labelField = catalog.defTypes.ThingDef.fields.label;
const tagsField = catalog.defTypes.ThingDef.fields.tags;
const compsField = catalog.defTypes.ThingDef.fields.comps;
const graphicDataField = catalog.defTypes.ThingDef.fields.graphicData;

function scalarView(overrides: Partial<XmlChildView> = {}): XmlChildView {
  return {
    nodeId: 1,
    name: "label",
    textValue: "Wall",
    listItems: [],
    xmlShape: "element",
    order: 0,
    known: true,
    line: null,
    column: null,
    ...overrides,
  };
}

describe("isStructurallySupportedField", () => {
  it("supports scalar, list, and object-list shapes", () => {
    expect(isStructurallySupportedField(labelField)).toBe(true);
    expect(isStructurallySupportedField(tagsField)).toBe(true);
    expect(isStructurallySupportedField(compsField)).toBe(true);
  });

  it("does not support reference fields", () => {
    expect(isStructurallySupportedField(graphicDataField)).toBe(false);
  });

  it("does not support keyedObjectList/keyedObjectMap fields, even though they classify as objectList", () => {
    // Both shapes resolve to the "objectList" control (see objectDescriptors.ts's
    // fieldSchemaToControl), but objectFieldValueToInitialElement always serializes an
    // "objectList" value as `<li Class="...">` items -- correct only for plain listOfLi. A
    // keyedObjectList field (e.g. BiomeDef.baseWeatherCommonalities) needs keyed child elements
    // like `<Rain>1</Rain>`, and keyedObjectMap needs `<li><key>/<value></li>` entries; treating
    // either as structurally supported would silently write invalid RimWorld XML.
    const keyedObjectList: FieldSchema = {
      type: { kind: "list" },
      required: false,
      examples: [],
      repeatable: false,
      xml: "keyedObjectList",
      flags: false,
      items: { kind: "object", schemaRef: "CompProperties" },
      keyField: "weatherDef",
    };
    const keyedObjectMap: FieldSchema = {
      type: { kind: "list" },
      required: false,
      examples: [],
      repeatable: false,
      xml: "keyedObjectMap",
      flags: false,
      items: { kind: "object", schemaRef: "CompProperties" },
    };
    expect(isStructurallySupportedField(keyedObjectList)).toBe(false);
    expect(isStructurallySupportedField(keyedObjectMap)).toBe(false);
  });
});

describe("parsedViewsToFieldValue", () => {
  it("reports an empty payload", () => {
    expect(parsedViewsToFieldValue([], "label", labelField, catalog)).toEqual({ kind: "empty" });
  });

  it("reports multiple top-level elements as unsupported", () => {
    const result = parsedViewsToFieldValue([scalarView(), scalarView()], "label", labelField, catalog);
    expect(result.kind).toBe("unsupportedShape");
  });

  it("reports a name mismatch instead of silently guessing", () => {
    const result = parsedViewsToFieldValue([scalarView({ name: "comps" })], "label", labelField, catalog);
    expect(result).toEqual({ kind: "mismatch", actualName: "comps" });
  });

  it("parses a scalar field payload", () => {
    const result = parsedViewsToFieldValue([scalarView()], "label", labelField, catalog);
    expect(result).toEqual({ kind: "ok", value: { kind: "scalar", value: "Wall" } });
  });

  it("parses a scalar list field payload", () => {
    const view: XmlChildView = {
      nodeId: 2,
      name: "tags",
      textValue: null,
      listItems: ["Alpha", "Beta"],
      xmlShape: "listOfLi",
      order: 0,
      known: true,
      line: null,
      column: null,
      liItems: [
        { nodeId: 20, textValue: "Alpha", attributes: [], children: [], order: 0, line: null, column: null, selfClosing: false },
        { nodeId: 21, textValue: "Beta", attributes: [], children: [], order: 1, line: null, column: null, selfClosing: false },
      ],
    };
    const result = parsedViewsToFieldValue([view], "tags", tagsField, catalog);
    expect(result).toEqual({ kind: "ok", value: { kind: "list", items: ["Alpha", "Beta"] } });
  });

  it("falls back to unsupported when list items carry their own attributes/children", () => {
    const view: XmlChildView = {
      nodeId: 3,
      name: "tags",
      textValue: null,
      listItems: [""],
      xmlShape: "listOfLi",
      order: 0,
      known: true,
      line: null,
      column: null,
      liItems: [
        {
          nodeId: 30,
          textValue: null,
          attributes: [{ name: "Class", value: "Something", known: false }],
          children: [],
          order: 0,
          line: null,
          column: null,
          selfClosing: false,
        },
      ],
    };
    const result = parsedViewsToFieldValue([view], "tags", tagsField, catalog);
    expect(result.kind).toBe("unsupportedShape");
  });

  it("parses an object list field payload with a Class discriminator", () => {
    const view: XmlChildView = {
      nodeId: 4,
      name: "comps",
      textValue: null,
      listItems: [],
      xmlShape: "listOfLi",
      order: 0,
      known: true,
      line: null,
      column: null,
      liItems: [
        {
          nodeId: 40,
          textValue: null,
          attributes: [{ name: "Class", value: "CompProperties_Foo", known: false }],
          children: [
            {
              nodeId: 41,
              name: "hitPoints",
              textValue: "10",
              listItems: [],
              xmlShape: "element",
              order: 0,
              line: null,
              column: null,
            },
          ],
          order: 0,
          line: null,
          column: null,
          selfClosing: false,
        },
      ],
    };
    const result = parsedViewsToFieldValue([view], "comps", compsField, catalog);
    expect(result.kind).toBe("ok");
    if (result.kind !== "ok") throw new Error("expected ok");
    expect(result.value).toMatchObject({
      kind: "objectList",
      itemSchemaRef: "CompProperties",
      items: [
        {
          className: "CompProperties_Foo",
          schemaRef: "CompProperties_Foo",
          fields: { hitPoints: { kind: "scalar", value: "10" } },
        },
      ],
    });
  });
});

describe("emptyFieldValue + fieldValueToInitialElement round trip", () => {
  it("builds a blank scalar value and serializes it back to an initial element", () => {
    const blank = emptyFieldValue("label", labelField, catalog);
    expect(blank).toEqual({ kind: "scalar", value: "" });
    expect(fieldValueToInitialElement("label", { kind: "scalar", value: "Wall" })).toEqual({
      name: "label",
      value: "Wall",
    });
    expect(fieldValueToInitialElement("label", blank)).toBeNull();
  });

  it("builds a blank object-list value ready for item entry", () => {
    const blank = emptyFieldValue("comps", compsField, catalog);
    expect(blank).toMatchObject({ kind: "objectList", itemSchemaRef: "CompProperties", items: [] });
  });
});
