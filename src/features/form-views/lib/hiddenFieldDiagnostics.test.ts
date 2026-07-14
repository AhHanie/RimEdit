import {
  collectDefSubtreeNodeIds,
  computeHiddenFieldDiagnosticsSummary,
  mapFieldPathToTopLevelRoot,
} from "./hiddenFieldDiagnostics";
import type { DefEditorView, ValidationDiagnostic } from "../../xml-editor/types/xmlDocument";

describe("mapFieldPathToTopLevelRoot", () => {
  it("maps a top-level scalar path to itself", () => {
    expect(mapFieldPathToTopLevelRoot("foo")).toBe("foo");
  });

  it("maps a nested object field path to its object root", () => {
    expect(mapFieldPathToTopLevelRoot("graphicData.texPath")).toBe("graphicData");
  });

  it("strips a list index suffix on the first segment", () => {
    expect(mapFieldPathToTopLevelRoot("comps[0].class")).toBe("comps");
  });

  it("strips a keyed map suffix with no dot at all", () => {
    expect(mapFieldPathToTopLevelRoot("statBases[WorkToMake]")).toBe("statBases");
  });

  it("handles multiple nesting levels, using only the first dot segment", () => {
    expect(mapFieldPathToTopLevelRoot("comps[0].nested[1].deep")).toBe("comps");
  });

  it("handles a keyed object-list `[li]` placeholder path", () => {
    expect(mapFieldPathToTopLevelRoot("comps[li].class")).toBe("comps");
  });

  it("returns null for an empty string", () => {
    expect(mapFieldPathToTopLevelRoot("")).toBeNull();
  });

  it("returns null for null/undefined", () => {
    expect(mapFieldPathToTopLevelRoot(null)).toBeNull();
    expect(mapFieldPathToTopLevelRoot(undefined)).toBeNull();
  });

  it("returns null for a bracket-only first segment (malformed/Def-level path)", () => {
    expect(mapFieldPathToTopLevelRoot("[0]")).toBeNull();
  });

  it("returns null for a dot-only path", () => {
    expect(mapFieldPathToTopLevelRoot(".")).toBeNull();
  });

  it("returns null for a whitespace-only path", () => {
    expect(mapFieldPathToTopLevelRoot("   ")).toBeNull();
  });

  it("tolerates a trailing dot", () => {
    expect(mapFieldPathToTopLevelRoot("foo.")).toBe("foo");
  });

  it("tolerates an unclosed/malformed bracket", () => {
    expect(mapFieldPathToTopLevelRoot("foo[0")).toBe("foo");
  });

  it("tolerates a bracket with no closing brace at all on a keyed path", () => {
    expect(mapFieldPathToTopLevelRoot("statBases[WorkToMake")).toBe("statBases");
  });
});

// --- Fixtures -----------------------------------------------------------------------------

function makeDef(overrides: Partial<DefEditorView> = {}): DefEditorView {
  return {
    nodeId: 1,
    defType: "ThingDef",
    defName: "Test",
    label: null,
    parentName: null,
    line: null,
    column: null,
    attributes: [],
    children: [],
    ...overrides,
  };
}

function diag(overrides: Partial<ValidationDiagnostic> = {}): ValidationDiagnostic {
  return {
    relativePath: "Things/Test.xml",
    nodeId: 1,
    line: null,
    column: null,
    severity: "Warning",
    message: "test",
    code: "test_code",
    defType: "ThingDef",
    defName: "Test",
    fieldPath: null,
    blocking: false,
    ...overrides,
  };
}

describe("collectDefSubtreeNodeIds", () => {
  it("includes the Def's own root node id", () => {
    const def = makeDef({ nodeId: 42 });
    expect(collectDefSubtreeNodeIds(def)).toEqual(new Set([42]));
  });

  it("includes direct children", () => {
    const def = makeDef({
      nodeId: 1,
      children: [
        { nodeId: 2, name: "defName", textValue: "Test", listItems: [], xmlShape: "element", order: 0, known: true, line: null, column: null },
        { nodeId: 3, name: "graphicData", textValue: null, listItems: [], xmlShape: "object", order: 1, known: true, line: null, column: null },
      ],
    });
    expect(collectDefSubtreeNodeIds(def)).toEqual(new Set([1, 2, 3]));
  });

  it("recurses into nested object children, list-of-li items, and keyed object list items", () => {
    const def = makeDef({
      nodeId: 1,
      children: [
        {
          nodeId: 2,
          name: "graphicData",
          textValue: null,
          listItems: [],
          xmlShape: "object",
          order: 0,
          known: true,
          line: null,
          column: null,
          children: [
            { nodeId: 3, name: "texPath", textValue: "a", listItems: [], xmlShape: "element", order: 0, line: null, column: null },
          ],
        },
        {
          nodeId: 4,
          name: "comps",
          textValue: null,
          listItems: [],
          xmlShape: "listOfLi",
          order: 1,
          known: true,
          line: null,
          column: null,
          liItems: [
            {
              nodeId: 5,
              textValue: null,
              attributes: [],
              children: [
                { nodeId: 6, name: "class", textValue: "Comp", listItems: [], xmlShape: "element", order: 0, line: null, column: null },
              ],
              order: 0,
              line: null,
              column: null,
              selfClosing: false,
            },
          ],
        },
        {
          nodeId: 7,
          name: "statBases",
          textValue: null,
          listItems: [],
          xmlShape: "namedChildrenMap",
          order: 2,
          known: true,
          line: null,
          column: null,
          liObjectItems: [
            [
              { nodeId: 8, name: "WorkToMake", textValue: "1", listItems: [], xmlShape: "element", order: 0, line: null, column: null },
            ],
          ],
        },
      ],
    });
    expect(collectDefSubtreeNodeIds(def)).toEqual(new Set([1, 2, 3, 4, 5, 6, 7, 8]));
  });
});

describe("computeHiddenFieldDiagnosticsSummary", () => {
  const def = makeDef({
    nodeId: 1,
    children: [
      { nodeId: 2, name: "defName", textValue: "Test", listItems: [], xmlShape: "element", order: 0, known: true, line: null, column: null },
      {
        nodeId: 3,
        name: "graphicData",
        textValue: null,
        listItems: [],
        xmlShape: "object",
        order: 1,
        known: true,
        line: null,
        column: null,
        children: [
          { nodeId: 4, name: "texPath", textValue: "bad value", listItems: [], xmlShape: "element", order: 0, line: null, column: null },
        ],
      },
    ],
  });

  it("returns an empty summary when nothing is hidden", () => {
    const summary = computeHiddenFieldDiagnosticsSummary({
      diagnostics: [diag({ nodeId: 4, fieldPath: "graphicData.texPath" })],
      def,
      effectiveHidden: new Set(),
    });
    expect(summary).toEqual({ affectedRootIds: new Set(), totalCount: 0, blockingCount: 0 });
  });

  it("counts a diagnostic whose mapped root is hidden", () => {
    const summary = computeHiddenFieldDiagnosticsSummary({
      diagnostics: [diag({ nodeId: 4, fieldPath: "graphicData.texPath", blocking: true })],
      def,
      effectiveHidden: new Set(["graphicData"]),
    });
    expect(summary.affectedRootIds).toEqual(new Set(["graphicData"]));
    expect(summary.totalCount).toBe(1);
    expect(summary.blockingCount).toBe(1);
  });

  it("ignores a diagnostic whose mapped root is visible (not in effectiveHidden)", () => {
    const summary = computeHiddenFieldDiagnosticsSummary({
      diagnostics: [diag({ nodeId: 4, fieldPath: "graphicData.texPath" })],
      def,
      effectiveHidden: new Set(["comps"]),
    });
    expect(summary.totalCount).toBe(0);
  });

  it("counts a required-missing-field diagnostic (nodeId equal to the Def's own root)", () => {
    const summary = computeHiddenFieldDiagnosticsSummary({
      diagnostics: [
        diag({ nodeId: 1, fieldPath: "graphicData", code: "validation_missing_required_field" }),
      ],
      def,
      effectiveHidden: new Set(["graphicData"]),
    });
    expect(summary.totalCount).toBe(1);
    expect(summary.affectedRootIds).toEqual(new Set(["graphicData"]));
  });

  it("excludes a diagnostic with no fieldPath (Def-level/unmapped) even if hidden roots exist", () => {
    const summary = computeHiddenFieldDiagnosticsSummary({
      diagnostics: [diag({ nodeId: 1, fieldPath: null, code: "validation_duplicate_def_name" })],
      def,
      effectiveHidden: new Set(["graphicData"]),
    });
    expect(summary.totalCount).toBe(0);
    expect(summary.affectedRootIds.size).toBe(0);
  });

  it("excludes a diagnostic with a null nodeId", () => {
    const summary = computeHiddenFieldDiagnosticsSummary({
      diagnostics: [diag({ nodeId: null, fieldPath: "graphicData.texPath" })],
      def,
      effectiveHidden: new Set(["graphicData"]),
    });
    expect(summary.totalCount).toBe(0);
  });

  it("excludes a diagnostic belonging to a different Def instance (nodeId outside this Def's subtree)", () => {
    const summary = computeHiddenFieldDiagnosticsSummary({
      diagnostics: [diag({ nodeId: 999, fieldPath: "graphicData.texPath" })],
      def,
      effectiveHidden: new Set(["graphicData"]),
    });
    expect(summary.totalCount).toBe(0);
  });

  it("mixes blocking and non-blocking diagnostics into separate counts", () => {
    const summary = computeHiddenFieldDiagnosticsSummary({
      diagnostics: [
        diag({ nodeId: 4, fieldPath: "graphicData.texPath", blocking: true }),
        diag({ nodeId: 4, fieldPath: "graphicData.graphicClass", blocking: false }),
      ],
      def,
      effectiveHidden: new Set(["graphicData"]),
    });
    expect(summary.totalCount).toBe(2);
    expect(summary.blockingCount).toBe(1);
    expect(summary.affectedRootIds).toEqual(new Set(["graphicData"]));
  });

  it("returns an empty summary when there are no diagnostics at all", () => {
    const summary = computeHiddenFieldDiagnosticsSummary({
      diagnostics: [],
      def,
      effectiveHidden: new Set(["graphicData"]),
    });
    expect(summary).toEqual({ affectedRootIds: new Set(), totalCount: 0, blockingCount: 0 });
  });
});
