import { classificationDiagnostics } from "./operationClassificationDiagnostics";
import type { SchemaCatalog } from "../../schema-catalog/types";
import type { PatchOperationNode } from "../types/patchFile";

function unknownNode(className: string, overrides: Partial<PatchOperationNode> = {}): PatchOperationNode {
  return {
    id: 0,
    className,
    success: "normal",
    attributes: [],
    kind: { type: "unknown", data: { rawXml: `<Operation Class="${className}"></Operation>` } },
    span: null,
    ...overrides,
  };
}

function builtInNode(): PatchOperationNode {
  return {
    id: 0,
    className: "PatchOperationAdd",
    success: "normal",
    attributes: [],
    kind: { type: "add", data: { xpath: null, valueXml: null, order: null } },
    span: null,
  };
}

function catalogWith(className: string): SchemaCatalog {
  return {
    formatVersion: 1,
    packs: [],
    defTypes: {},
    objectTypes: {},
    patchOperations: {
      [className]: {
        className,
        label: "Custom Op",
        fieldOrder: [],
        fields: {},
        preview: { kind: "unsupported", message: "Preview cannot execute this custom operation." },
      },
    },
  };
}

describe("classificationDiagnostics", () => {
  it("reports no diagnostics for built-in (known-kind) operations", () => {
    expect(classificationDiagnostics([builtInNode()], null)).toEqual([]);
  });

  it("reports a genuinely-unknown class as unrecognized when the catalog has no metadata for it", () => {
    const diagnostics = classificationDiagnostics([unknownNode("Totally.Bogus.Class")], null);
    expect(diagnostics).toHaveLength(1);
    expect(diagnostics[0].message).toContain("not a recognized built-in patch operation class");
    expect(diagnostics[0].message).toContain("Totally.Bogus.Class");
  });

  it("reports a metadata-known custom class as unpreviewable rather than unrecognized", () => {
    const catalog = catalogWith("MyMod.PatchOperationFoo");
    const diagnostics = classificationDiagnostics(
      [unknownNode("MyMod.PatchOperationFoo")],
      catalog,
    );
    expect(diagnostics).toHaveLength(1);
    expect(diagnostics[0].message).toContain("custom operation defined by schema-pack metadata");
    expect(diagnostics[0].message).toContain("Preview cannot execute this custom operation.");
  });

  it("does not report a diagnostic for a node with no Class attribute (already covered by the parser's own diagnostic)", () => {
    expect(classificationDiagnostics([unknownNode("")], null)).toEqual([]);
  });

  it("does not report a diagnostic for a built-in class that fell back to raw XML because of an unrecognized field", () => {
    // `patches::parser` falls a *known* built-in class back to `unknown` (raw XML) when it has an
    // extra field the typed model doesn't recognize, and pushes its own diagnostic explaining
    // that. Built-in classes are also shipped as schema-pack metadata (so the form renderer is
    // data-driven), so this must not be mistaken for "a custom operation the catalog knows about".
    const catalog = catalogWith("PatchOperationAdd");
    const diagnostics = classificationDiagnostics([unknownNode("PatchOperationAdd")], catalog);
    expect(diagnostics).toEqual([]);
  });

  it("recurses into sequence, findMod, and conditional nested operations", () => {
    const nested: PatchOperationNode = {
      id: 1,
      className: "PatchOperationSequence",
      success: "normal",
      attributes: [],
      kind: { type: "sequence", data: [unknownNode("Nested.Bogus")] },
      span: null,
    };
    const findMod: PatchOperationNode = {
      id: 2,
      className: "PatchOperationFindMod",
      success: "normal",
      attributes: [],
      kind: {
        type: "findMod",
        data: { mods: [], matchOp: unknownNode("Match.Bogus"), nomatchOp: unknownNode("Nomatch.Bogus") },
      },
      span: null,
    };

    const diagnostics = classificationDiagnostics([nested, findMod], null);
    expect(diagnostics).toHaveLength(3);
    expect(diagnostics.map((d) => d.message).join("\n")).toContain("Nested.Bogus");
    expect(diagnostics.map((d) => d.message).join("\n")).toContain("Match.Bogus");
    expect(diagnostics.map((d) => d.message).join("\n")).toContain("Nomatch.Bogus");
  });
});
