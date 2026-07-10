import { buildEffectiveFieldOrder, buildNestedFieldOrders } from "./schemaFieldOrder";
import type { FieldSchema, SchemaCatalog } from "../../schema-catalog";

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

describe("buildEffectiveFieldOrder", () => {
  it("returns element fields in ancestor-first order", () => {
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {},
      defTypes: {
        ThingDef: {
          inherits: ["Def"],
          abstractType: false,
          fieldOrder: ["stackLimit"],
          fields: { stackLimit: makeField({ xml: "element" }) },
        },
        Def: {
          inherits: [],
          abstractType: true,
          fieldOrder: ["defName", "label"],
          fields: {
            defName: makeField({ xml: "element" }),
            label: makeField({ xml: "element" }),
          },
        },
      },
    };
    const order = buildEffectiveFieldOrder("ThingDef", catalog);
    expect(order).toEqual(["defName", "label", "stackLimit"]);
  });

  it("excludes attribute and text fields", () => {
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {},
      defTypes: {
        ThingDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: ["ParentName", "defName", "body"],
          fields: {
            ParentName: makeField({ xml: "attribute" }),
            defName: makeField({ xml: "element" }),
            body: makeField({ xml: "text" }),
          },
        },
      },
    };
    const order = buildEffectiveFieldOrder("ThingDef", catalog);
    expect(order).toEqual(["defName"]);
  });

  it("includes listOfLi, namedChildrenMap, and keyedValueList shapes", () => {
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {},
      defTypes: {
        ThingDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: ["defName", "ingredients", "statBases", "skillRequirements"],
          fields: {
            defName: makeField({ xml: "element" }),
            ingredients: makeField({ xml: "listOfLi" }),
            statBases: makeField({ xml: "namedChildrenMap" }),
            skillRequirements: makeField({ xml: "keyedValueList" }),
          },
        },
      },
    };
    const order = buildEffectiveFieldOrder("ThingDef", catalog);
    expect(order).toEqual(["defName", "ingredients", "statBases", "skillRequirements"]);
  });

  it("appends fields not in fieldOrder as fallback", () => {
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {},
      defTypes: {
        ThingDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: ["defName"],
          fields: {
            defName: makeField({ xml: "element" }),
            label: makeField({ xml: "element" }),
          },
        },
      },
    };
    const order = buildEffectiveFieldOrder("ThingDef", catalog);
    expect(order).toContain("defName");
    expect(order).toContain("label");
    expect(order.indexOf("defName")).toBeLessThan(order.indexOf("label"));
  });

  it("returns empty array for unknown def type", () => {
    const catalog: SchemaCatalog = { formatVersion: 1, packs: [], defTypes: {}, objectTypes: {} };
    expect(buildEffectiveFieldOrder("NonExistent", catalog)).toEqual([]);
  });
});

describe("buildNestedFieldOrders", () => {
  it("builds object path order for ThingDef.graphicData", () => {
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {
        GraphicData: {
          fieldOrder: ["texPath", "graphicClass", "color"],
          fields: {
            texPath: makeField({ xml: "element" }),
            graphicClass: makeField({ xml: "element" }),
            color: makeField({ xml: "attribute" }),
          },
        },
      },
      defTypes: {
        ThingDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: ["defName", "graphicData"],
          fields: {
            defName: makeField({ xml: "element" }),
            graphicData: makeField({ type: { kind: "object", schemaRef: "GraphicData" }, xml: "object" }),
          },
        },
      },
    };
    const orders = buildNestedFieldOrders("ThingDef", catalog);
    expect(orders["graphicData"]).toEqual(["texPath", "graphicClass"]);
    expect(orders["graphicData"]).not.toContain("color");
  });

  it("recurses into nested object types when schemaRef is present", () => {
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {
        GraphicData: {
          fieldOrder: ["texPath", "shadowData"],
          fields: {
            texPath: makeField({ xml: "element" }),
            shadowData: makeField({ type: { kind: "object", schemaRef: "ShadowData" }, xml: "element" }),
          },
        },
        ShadowData: {
          fieldOrder: ["volume", "offset"],
          fields: {
            volume: makeField({ xml: "element" }),
            offset: makeField({ xml: "element" }),
          },
        },
      },
      defTypes: {
        ThingDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: ["graphicData"],
          fields: {
            graphicData: makeField({ type: { kind: "object", schemaRef: "GraphicData" }, xml: "object" }),
          },
        },
      },
    };
    const orders = buildNestedFieldOrders("ThingDef", catalog);
    expect(orders["graphicData"]).toEqual(["texPath", "shadowData"]);
    expect(orders["graphicData.shadowData"]).toEqual(["volume", "offset"]);
  });

  it("excludes nested attribute and text fields from element insertion order", () => {
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {
        GraphicData: {
          fieldOrder: ["texPath", "color", "body"],
          fields: {
            texPath: makeField({ xml: "element" }),
            color: makeField({ xml: "attribute" }),
            body: makeField({ xml: "text" }),
          },
        },
      },
      defTypes: {
        ThingDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: ["graphicData"],
          fields: {
            graphicData: makeField({ type: { kind: "object", schemaRef: "GraphicData" }, xml: "object" }),
          },
        },
      },
    };
    const orders = buildNestedFieldOrders("ThingDef", catalog);
    expect(orders["graphicData"]).toEqual(["texPath"]);
  });

  it("appends object fields not in fieldOrder", () => {
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {
        GraphicData: {
          fieldOrder: ["texPath"],
          fields: {
            texPath: makeField({ xml: "element" }),
            graphicClass: makeField({ xml: "element" }),
          },
        },
      },
      defTypes: {
        ThingDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: ["graphicData"],
          fields: {
            graphicData: makeField({ type: { kind: "object", schemaRef: "GraphicData" }, xml: "object" }),
          },
        },
      },
    };
    const orders = buildNestedFieldOrders("ThingDef", catalog);
    expect(orders["graphicData"]).toContain("texPath");
    expect(orders["graphicData"]).toContain("graphicClass");
    expect(orders["graphicData"].indexOf("texPath")).toBeLessThan(orders["graphicData"].indexOf("graphicClass"));
  });

  it("avoids infinite cycles in mutually-referencing object types", () => {
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {
        A: {
          fieldOrder: ["b"],
          fields: {
            b: makeField({ type: { kind: "object", schemaRef: "B" }, xml: "element" }),
          },
        },
        B: {
          fieldOrder: ["a"],
          fields: {
            a: makeField({ type: { kind: "object", schemaRef: "A" }, xml: "element" }),
          },
        },
      },
      defTypes: {
        ThingDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: ["root"],
          fields: {
            root: makeField({ type: { kind: "object", schemaRef: "A" }, xml: "object" }),
          },
        },
      },
    };
    expect(() => buildNestedFieldOrders("ThingDef", catalog)).not.toThrow();
  });

  it("returns empty object when def type has no schema-backed object fields", () => {
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {},
      defTypes: {
        ThingDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: ["defName"],
          fields: { defName: makeField({ xml: "element" }) },
        },
      },
    };
    expect(buildNestedFieldOrders("ThingDef", catalog)).toEqual({});
  });
});

describe("buildEffectiveFieldOrder – flagsText shape", () => {
  it("includes flagsText fields in effective field order", () => {
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {},
      defTypes: {
        ThingDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: ["defName", "developmentalStageFilter"],
          fields: {
            defName: makeField({ xml: "element" }),
            developmentalStageFilter: makeField({ xml: "flagsText" }),
          },
        },
      },
    };
    const order = buildEffectiveFieldOrder("ThingDef", catalog);
    expect(order).toContain("developmentalStageFilter");
    expect(order.indexOf("defName")).toBeLessThan(order.indexOf("developmentalStageFilter"));
  });

  it("excludes flagsText fields from nested field orders (not an object container)", () => {
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      objectTypes: {
        ApparelProperties: {
          fieldOrder: ["developmentalStageFilter", "countsAsClothingForNudity"],
          fields: {
            developmentalStageFilter: makeField({ xml: "flagsText" }),
            countsAsClothingForNudity: makeField({ xml: "element" }),
          },
        },
      },
      defTypes: {
        ThingDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: ["apparel"],
          fields: {
            apparel: makeField({ type: { kind: "object", schemaRef: "ApparelProperties" }, xml: "object" }),
          },
        },
      },
    };
    const orders = buildNestedFieldOrders("ThingDef", catalog);
    // flagsText is an element-child shape so must be included in the object's field order
    expect(orders["apparel"]).toContain("developmentalStageFilter");
    expect(orders["apparel"]).toContain("countsAsClothingForNudity");
  });
});
