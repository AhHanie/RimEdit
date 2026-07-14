import { act, renderHook } from "@testing-library/react";
import { vi, describe, it, expect } from "vitest";
import * as formDescriptorsModule from "../lib/formDescriptors";
import { useXmlFormController } from "./useXmlFormController";
import type { XmlEditorSnapshot } from "../types/editorSession";
import type { SchemaCatalog } from "../../schema-catalog";

// Issue 05 review round 2, finding 3: `models`/`descriptors` must depend on the content-
// stable id (mirroring `getCatalogId`'s role for catalog identity), not the raw
// `visibleTopLevelFieldIds` Set (or `catalog`) reference - otherwise a caller that
// re-creates a content-equal Set/catalog on every render (a common React pattern, e.g.
// `new Set(someComputedArray)` inline) would force the expensive descriptor rebuild/nested
// expansion this issue's filtering exists to avoid, on every single render.
//
// This file mocks `../lib/formDescriptors` (wrapping the real implementations in `vi.fn` so
// behavior is unchanged but calls are countable) rather than adding this to the large shared
// `useXmlFormController.test.tsx`, so the module-level `vi.mock` here cannot affect any other
// test file's assertions.
vi.mock("../lib/formDescriptors", async (importOriginal) => {
  const actual =
    await importOriginal<typeof import("../lib/formDescriptors")>();
  return {
    ...actual,
    buildFormFieldModels: vi.fn(actual.buildFormFieldModels),
    buildFormDescriptors: vi.fn(actual.buildFormDescriptors),
  };
});

function makeSnapshot(): XmlEditorSnapshot {
  return {
    rawXml:
      "<Defs><ThingDef><defName>Steel</defName><description>Old</description></ThingDef></Defs>",
    parseDiagnostics: [],
    validationDiagnostics: [],
    selectedDefNodeId: 1,
    parsed: {
      nodeCount: 3,
      rootElement: "Defs",
      profile: "defs",
      about: null,
      defs: [
        {
          nodeId: 1,
          defType: "ThingDef",
          defName: "Steel",
          label: null,
          parentName: null,
          line: null,
          column: null,
          attributes: [],
          children: [
            {
              nodeId: 2,
              name: "defName",
              textValue: "Steel",
              listItems: [],
              xmlShape: "element",
              order: 0,
              known: false,
              line: null,
              column: null,
            },
            {
              nodeId: 3,
              name: "description",
              textValue: "Old",
              listItems: [],
              xmlShape: "element",
              order: 1,
              known: false,
              line: null,
              column: null,
            },
          ],
        },
      ],
    },
  };
}

function makeCatalog(): SchemaCatalog {
  return {
    formatVersion: 1,
    packs: [],
    objectTypes: {},
    defTypes: {
      ThingDef: {
        inherits: [],
        abstractType: false,
        fieldOrder: [],
        fields: {
          defName: {
            type: { kind: "string" },
            required: false,
            repeatable: false,
            xml: "element",
            examples: [],
            flags: false,
          },
          description: {
            type: { kind: "string" },
            required: false,
            repeatable: false,
            xml: "element",
            examples: [],
            flags: false,
          },
        },
      },
    },
  };
}

describe("useXmlFormController – descriptor/model memo stability (issue 05 review finding 3)", () => {
  type Props = Parameters<typeof useXmlFormController>[0];

  it("does not rebuild descriptors/models when rerendered with a content-equal but new-instance visibility Set", () => {
    const buildModelsSpy = vi.mocked(formDescriptorsModule.buildFormFieldModels);
    const buildDescriptorsSpy = vi.mocked(
      formDescriptorsModule.buildFormDescriptors,
    );
    buildModelsSpy.mockClear();
    buildDescriptorsSpy.mockClear();

    const initialProps: Props = {
      snapshot: makeSnapshot(),
      catalog: makeCatalog(),
      selectedDefNodeId: 1,
      commitEdits: async () => "<xml/>",
      clearPreview: vi.fn(),
      visibleTopLevelFieldIds: new Set(["defName", "description"]),
    };
    const { rerender } = renderHook((p: Props) => useXmlFormController(p), {
      initialProps,
    });

    const modelCallsAfterMount = buildModelsSpy.mock.calls.length;
    const descriptorCallsAfterMount = buildDescriptorsSpy.mock.calls.length;
    expect(modelCallsAfterMount).toBeGreaterThan(0);
    expect(descriptorCallsAfterMount).toBeGreaterThan(0);

    act(() => {
      rerender({
        ...initialProps,
        // Brand-new Set instance, identical content - the pattern an unmemoized caller
        // (e.g. `new Set(someComputedArray)` inline every render) would produce.
        visibleTopLevelFieldIds: new Set(["defName", "description"]),
      });
    });

    expect(buildModelsSpy.mock.calls.length).toBe(modelCallsAfterMount);
    expect(buildDescriptorsSpy.mock.calls.length).toBe(descriptorCallsAfterMount);
  });

  it("does not rebuild descriptors/models when rerendered with a content-equal but new-instance catalog", () => {
    const buildModelsSpy = vi.mocked(formDescriptorsModule.buildFormFieldModels);
    buildModelsSpy.mockClear();

    const initialProps: Props = {
      snapshot: makeSnapshot(),
      catalog: makeCatalog(),
      selectedDefNodeId: 1,
      commitEdits: async () => "<xml/>",
      clearPreview: vi.fn(),
    };
    const { rerender } = renderHook((p: Props) => useXmlFormController(p), {
      initialProps,
    });

    const callsAfterMount = buildModelsSpy.mock.calls.length;
    expect(callsAfterMount).toBeGreaterThan(0);

    act(() => {
      // Brand-new catalog object, identical content.
      rerender({ ...initialProps, catalog: makeCatalog() });
    });

    expect(buildModelsSpy.mock.calls.length).toBe(callsAfterMount);
  });

  it("still rebuilds descriptors/models when the visibility content genuinely changes", () => {
    const buildModelsSpy = vi.mocked(formDescriptorsModule.buildFormFieldModels);
    buildModelsSpy.mockClear();

    const initialProps: Props = {
      snapshot: makeSnapshot(),
      catalog: makeCatalog(),
      selectedDefNodeId: 1,
      commitEdits: async () => "<xml/>",
      clearPreview: vi.fn(),
      visibleTopLevelFieldIds: new Set(["defName", "description"]),
    };
    const { rerender } = renderHook((p: Props) => useXmlFormController(p), {
      initialProps,
    });

    const callsAfterMount = buildModelsSpy.mock.calls.length;

    act(() => {
      rerender({
        ...initialProps,
        visibleTopLevelFieldIds: new Set(["defName"]),
      });
    });

    expect(buildModelsSpy.mock.calls.length).toBeGreaterThan(callsAfterMount);
  });
});
