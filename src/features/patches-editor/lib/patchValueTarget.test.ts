import { describe, expect, it } from "vitest";
import type { SchemaCatalog } from "../../schema-catalog";
import { listDirectDefTypeFields, resolveModExtensionsField, targetDefType } from "./patchValueTarget";

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
      fieldOrder: ["label", "comps"],
      fields: {
        label: {
          type: { kind: "string" },
          required: false,
          examples: [],
          repeatable: false,
          xml: "element",
          flags: false,
        },
        comps: {
          type: { kind: "list" },
          required: false,
          examples: [],
          repeatable: false,
          xml: "listOfLi",
          flags: false,
        },
      },
    },
  },
  objectTypes: {},
};

describe("listDirectDefTypeFields", () => {
  it("lists only fields declared directly on the concrete Def type, not its schema parents", () => {
    const fields = listDirectDefTypeFields("ThingDef", catalog).map(([name]) => name);
    expect(fields).toEqual(["label", "comps"]);
    expect(fields).not.toContain("modExtensions");
  });

  it("returns an empty list for an unknown Def type or null catalog", () => {
    expect(listDirectDefTypeFields("NoSuchDef", catalog)).toEqual([]);
    expect(listDirectDefTypeFields("ThingDef", null)).toEqual([]);
  });
});

describe("resolveModExtensionsField", () => {
  it("resolves modExtensions from the universal Def base type", () => {
    const field = resolveModExtensionsField(catalog);
    expect(field?.xml).toBe("listOfLi");
  });

  it("returns null when the catalog has no Def type at all", () => {
    expect(resolveModExtensionsField({ ...catalog, defTypes: {} })).toBeNull();
    expect(resolveModExtensionsField(null)).toBeNull();
  });
});

describe("targetDefType", () => {
  it("extracts defType from def and defType targets", () => {
    expect(targetDefType({ kind: "def", defType: "ThingDef", defName: "Wall" })).toBe("ThingDef");
    expect(targetDefType({ kind: "defType", defType: "ThingDef" })).toBe("ThingDef");
  });

  it("returns null for unsupported/noXPath/null targets", () => {
    expect(targetDefType({ kind: "unsupported" })).toBeNull();
    expect(targetDefType({ kind: "noXPath" })).toBeNull();
    expect(targetDefType(null)).toBeNull();
  });
});
