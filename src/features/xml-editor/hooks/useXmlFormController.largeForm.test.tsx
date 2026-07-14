// Form Views (issue 10, Plan.md section 10/13): count-based, non-timing performance-shape
// regression guards against the same large synthetic ThingDef-shaped fixture as
// `formDescriptors.largeForm.test.ts`, at the `useXmlFormController` store/rebuild layer.
// Existing `useXmlFormController.test.tsx` "Form View visibility filtering" tests already prove
// rebuild-once, draft-preservation, and no-XML-commit-on-visibility-change correctness at a
// 3-field toy scale; these tests prove the identical guarantees hold at realistic ThingDef scale
// (135 top-level fields, exactly 210 descriptors -- see `LARGE_FULL_DESCRIPTOR_COUNT`), with
// concrete counts as the regression guard.
import { act, renderHook } from "@testing-library/react";
import { useXmlFormController } from "./useXmlFormController";
import { FormFieldStore } from "../lib/formFieldStore";
import type { XmlEditorSnapshot } from "../types/editorSession";
import {
  buildLargeThingDefCatalog,
  buildLargeThingDefEditorView,
  bulkObjectAndListRootIds,
  LARGE_FULL_DESCRIPTOR_COUNT,
  LARGE_SCALAR_COUNT,
  allTopLevelFieldIds,
  scalarFieldId,
  scalarsOnlyVisibleSet,
} from "../__fixtures__/largeThingDef";

function makeLargeSnapshot(): XmlEditorSnapshot {
  const def = buildLargeThingDefEditorView();
  return {
    rawXml: "<Defs><ThingDef>(large synthetic fixture)</ThingDef></Defs>",
    parseDiagnostics: [],
    validationDiagnostics: [],
    selectedDefNodeId: def.nodeId,
    parsed: {
      nodeCount: def.children.length + 1,
      rootElement: "Defs",
      profile: "defs",
      about: null,
      defs: [def],
    },
  };
}

describe("useXmlFormController - large synthetic ThingDef-shaped form (issue 10)", () => {
  type Props = Parameters<typeof useXmlFormController>[0];

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("constructs the full model set for the large form on mount", () => {
    const { result } = renderHook(() =>
      useXmlFormController({
        snapshot: makeLargeSnapshot(),
        catalog: buildLargeThingDefCatalog(),
        selectedDefNodeId: buildLargeThingDefEditorView().nodeId,
        commitEdits: async () => "<xml/>",
        clearPreview: vi.fn(),
      }),
    );

    expect(result.current.snapshot!.fields.length).toBe(LARGE_FULL_DESCRIPTOR_COUNT);
  });

  it("hiding the bulk of the field surface in one view switch rebuilds the store exactly once, never invokes commitEdits, and drops the model count to exactly the scalar-only count", () => {
    const resetSpy = vi.spyOn(FormFieldStore.prototype, "reset");
    const commitEdits = vi.fn(async () => "<xml/>");
    const initialProps: Props = {
      snapshot: makeLargeSnapshot(),
      catalog: buildLargeThingDefCatalog(),
      selectedDefNodeId: buildLargeThingDefEditorView().nodeId,
      commitEdits,
      clearPreview: vi.fn(),
      visibleTopLevelFieldIds: new Set(allTopLevelFieldIds()),
    };
    const { result, rerender } = renderHook((p: Props) => useXmlFormController(p), {
      initialProps,
    });
    expect(result.current.snapshot!.fields.length).toBe(LARGE_FULL_DESCRIPTOR_COUNT);
    resetSpy.mockClear();

    act(() => {
      // A single Form View switch that hides every object/list root at once - the same shape
      // of change a real "minimal" schema view would make in one selection, not N incremental
      // per-field toggles.
      rerender({ ...initialProps, visibleTopLevelFieldIds: scalarsOnlyVisibleSet() });
    });

    expect(resetSpy).toHaveBeenCalledTimes(1);
    expect(result.current.snapshot!.fields.length).toBe(LARGE_SCALAR_COUNT);
    expect(commitEdits).not.toHaveBeenCalled();
    for (const id of bulkObjectAndListRootIds()) {
      expect(result.current.snapshot!.fields.some((f) => f.model.key === id)).toBe(false);
    }
  });

  it("typing into one remaining visible scalar field only marks that field dirty - sibling visible fields and stashed-hidden fields are untouched", () => {
    const initialProps: Props = {
      snapshot: makeLargeSnapshot(),
      catalog: buildLargeThingDefCatalog(),
      selectedDefNodeId: buildLargeThingDefEditorView().nodeId,
      commitEdits: async () => "<xml/>",
      clearPreview: vi.fn(),
      visibleTopLevelFieldIds: scalarsOnlyVisibleSet(),
    };
    const { result } = renderHook((p: Props) => useXmlFormController(p), { initialProps });

    const targetId = result.current.snapshot!.fields.find(
      (f) => f.model.key === scalarFieldId(0),
    )!.model.id;

    act(() => {
      result.current.setFieldValue(targetId, { kind: "scalar", value: "edited" });
    });

    const allFields = result.current.snapshot!.fields;
    const dirtyIds = allFields.filter((f) => f.dirty).map((f) => f.model.id);
    expect(dirtyIds).toEqual([targetId]);

    // A sample of other visible scalar fields stayed exactly as they were.
    const untouched = allFields.find((f) => f.model.key === scalarFieldId(1))!;
    expect(untouched.dirty).toBe(false);
    expect(untouched.value).toEqual({ kind: "scalar", value: "value-1" });
  });

  it("switching back to the full view after hiding the bulk of the field surface restores every previously-hidden field's original value, still without ever invoking commitEdits", () => {
    const commitEdits = vi.fn(async () => "<xml/>");
    const initialProps: Props = {
      snapshot: makeLargeSnapshot(),
      catalog: buildLargeThingDefCatalog(),
      selectedDefNodeId: buildLargeThingDefEditorView().nodeId,
      commitEdits,
      clearPreview: vi.fn(),
      visibleTopLevelFieldIds: new Set(allTopLevelFieldIds()),
    };
    const { result, rerender } = renderHook((p: Props) => useXmlFormController(p), {
      initialProps,
    });

    act(() => {
      rerender({ ...initialProps, visibleTopLevelFieldIds: scalarsOnlyVisibleSet() });
    });
    expect(result.current.snapshot!.fields.length).toBe(LARGE_SCALAR_COUNT);

    act(() => {
      rerender({ ...initialProps, visibleTopLevelFieldIds: new Set(allTopLevelFieldIds()) });
    });

    expect(result.current.snapshot!.fields.length).toBe(LARGE_FULL_DESCRIPTOR_COUNT);
    expect(commitEdits).not.toHaveBeenCalled();

    // A sample of previously-hidden fields (one object-root nested field, one plain list root)
    // is back with its original value and clean state - not merely present again.
    const restoredNested = result.current.snapshot!.fields.find(
      (f) => f.model.fieldPath.join(".") === "objectRoot0.nested0",
    );
    expect(restoredNested).toBeDefined();
    expect(restoredNested!.value).toEqual({ kind: "scalar", value: "obj0-nested0" });
    expect(restoredNested!.dirty).toBe(false);

    const restoredList = result.current.snapshot!.fields.find(
      (f) => f.model.key === "listRoot4", // a plain (non-object) list root
    );
    expect(restoredList).toBeDefined();
    expect(restoredList!.value).toEqual({
      kind: "list",
      items: ["list4-a", "list4-b"],
    });
  });
});
