import { act, renderHook, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { useFormViews } from "./useFormViews";
import type { SchemaCatalog } from "../../schema-catalog";
import type { CustomFormView } from "../types/formViews";
import { buildFormFieldModels } from "../../xml-editor/lib/formDescriptors";
import type { DefEditorView } from "../../xml-editor/types/xmlDocument";
import { useXmlFormController } from "../../xml-editor/hooks/useXmlFormController";
import type { XmlEditorSnapshot } from "../../xml-editor/types/editorSession";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

function makeCatalog(): SchemaCatalog {
  return {
    formatVersion: 1,
    packs: [],
    defTypes: {
      ThingDef: {
        inherits: [],
        abstractType: false,
        fieldOrder: ["defName", "apparel", "plant", "graphicData"],
        fields: {
          defName: { type: { kind: "string" }, required: true, examples: [], repeatable: false, xml: "element", flags: false },
          apparel: { type: { kind: "object" }, required: false, examples: [], repeatable: false, xml: "object", flags: false },
          plant: { type: { kind: "object" }, required: false, examples: [], repeatable: false, xml: "object", flags: false },
          graphicData: { type: { kind: "object" }, required: false, examples: [], repeatable: false, xml: "object", flags: false },
        },
        formViews: {
          weapon: {
            id: "weapon",
            label: "Weapon",
            order: 10,
            recommended: true,
            hiddenFieldIds: ["apparel", "plant"],
            declaredOnDefType: "ThingDef",
          },
        },
      },
      // A Def type with no `formViews` at all, exercising the "no schema views" fallback path.
      RecipeDef: {
        inherits: [],
        abstractType: false,
        fieldOrder: ["defName"],
        fields: {
          defName: { type: { kind: "string" }, required: true, examples: [], repeatable: false, xml: "element", flags: false },
        },
      },
    },
    objectTypes: {},
  };
}

function mockInvoke(overrides: Record<string, (args: unknown) => unknown> = {}) {
  invokeMock.mockImplementation((cmd: string, args?: unknown) => {
    if (cmd in overrides) return Promise.resolve(overrides[cmd](args));
    if (cmd === "list_custom_form_views") return Promise.resolve({ views: [], warning: null });
    if (cmd === "get_last_selected_form_view") return Promise.resolve({ selected: null, warning: null });
    if (cmd === "set_last_selected_form_view") return Promise.resolve(undefined);
    if (cmd === "create_custom_form_view") {
      const a = args as { name: string; hiddenFieldIds: string[] };
      const view: CustomFormView = {
        id: "new-custom",
        target: { gameVersion: "1.6", defType: "ThingDef" },
        name: a.name,
        description: null,
        hiddenFieldIds: a.hiddenFieldIds,
        baseSchemaView: null,
        createdAt: "2026-01-01T00:00:00Z",
        updatedAt: "2026-01-01T00:00:00Z",
      };
      return Promise.resolve(view);
    }
    return Promise.reject(new Error(`unexpected invoke: ${cmd}`));
  });
}

describe("useFormViews", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("resolves to the recommended schema view by default, and Default when no schema view exists", async () => {
    mockInvoke();
    const catalog = makeCatalog();

    const { result, rerender } = renderHook(
      (props: { defType: string }) =>
        useFormViews({
          projectId: "proj1",
          gameVersion: "1.6",
          catalog,
          pane: { locationId: "proj1", relativePath: "Defs/a.xml", sourceKind: "project" },
          selectedDef: { defType: props.defType, ordinal: 0 },
        }),
      { initialProps: { defType: "ThingDef" } },
    );

    await waitFor(() => expect(result.current.selectedView.id).toBe("weapon"));
    expect(result.current.applicable).toBe(true);
    expect(result.current.availableViews.map((v) => v.id)).toEqual(["default", "weapon"]);

    rerender({ defType: "RecipeDef" });
    await waitFor(() => expect(result.current.selectedView.origin).toBe("default"));
    expect(result.current.availableViews).toHaveLength(1);
  });

  it("is not applicable for a Def type absent from the catalog", async () => {
    mockInvoke();
    const catalog = makeCatalog();
    const { result } = renderHook(() =>
      useFormViews({
        projectId: "proj1",
        gameVersion: "1.6",
        catalog,
        pane: null,
        selectedDef: { defType: "UnknownDef", ordinal: 0 },
      }),
    );
    expect(result.current.applicable).toBe(false);
    expect(result.current.visibleTopLevelFieldIds).toBeNull();
  });

  it("computes visibleTopLevelFieldIds by intersecting the selected view's hidden set with the known field universe", async () => {
    mockInvoke();
    const catalog = makeCatalog();
    const { result } = renderHook(() =>
      useFormViews({
        projectId: "proj1",
        gameVersion: "1.6",
        catalog,
        pane: null,
        selectedDef: { defType: "ThingDef", ordinal: 0 },
      }),
    );

    await waitFor(() => expect(result.current.selectedView.id).toBe("weapon"));
    // ThingDef's known fields are defName/apparel/plant/graphicData; "weapon" hides apparel/plant.
    expect([...(result.current.visibleTopLevelFieldIds ?? [])].sort()).toEqual([
      "defName",
      "graphicData",
    ]);
    expect([...result.current.effectiveHidden].sort()).toEqual(["apparel", "plant"]);
  });

  it("honors a persisted custom-view selection over the recommended schema view", async () => {
    const customView: CustomFormView = {
      id: "custom-1",
      target: { gameVersion: "1.6", defType: "ThingDef" },
      name: "My view",
      description: null,
      hiddenFieldIds: ["graphicData"],
      baseSchemaView: null,
      createdAt: "2026-01-01T00:00:00Z",
      updatedAt: "2026-01-01T00:00:00Z",
    };
    mockInvoke({
      list_custom_form_views: () => ({ views: [customView], warning: null }),
      get_last_selected_form_view: () => ({
        selected: { origin: "custom", id: "custom-1" },
        warning: null,
      }),
    });
    const catalog = makeCatalog();
    const { result } = renderHook(() =>
      useFormViews({
        projectId: "proj1",
        gameVersion: "1.6",
        catalog,
        pane: null,
        selectedDef: { defType: "ThingDef", ordinal: 0 },
      }),
    );

    await waitFor(() => expect(result.current.selectedView.origin).toBe("custom"));
    expect(result.current.selectedView.id).toBe("custom-1");
    expect([...result.current.effectiveHidden]).toEqual(["graphicData"]);
  });

  it("respects a different game version's own persisted selection instead of falling back to Default and clobbering it", async () => {
    const customA: CustomFormView = {
      id: "custom-a",
      target: { gameVersion: "1.6", defType: "ThingDef" },
      name: "Version A view",
      description: null,
      hiddenFieldIds: ["graphicData"],
      baseSchemaView: null,
      createdAt: "2026-01-01T00:00:00Z",
      updatedAt: "2026-01-01T00:00:00Z",
    };
    const customB: CustomFormView = {
      id: "custom-b",
      target: { gameVersion: "1.5", defType: "ThingDef" },
      name: "Version B view",
      description: null,
      hiddenFieldIds: ["plant"],
      baseSchemaView: null,
      createdAt: "2026-01-01T00:00:00Z",
      updatedAt: "2026-01-01T00:00:00Z",
    };

    // Each game version scope has its OWN genuinely-different custom view list and persisted
    // selection -- exactly the `{project, gameVersion, defType}` scoping Plan.md section 3/6
    // requires. A correct implementation must fetch each scope's real data independently rather
    // than reusing whatever was already loaded for a previously-active version.
    invokeMock.mockImplementation((cmd: string, args?: unknown) => {
      const a = args as { gameVersion?: string } | undefined;
      if (cmd === "list_custom_form_views") {
        return Promise.resolve({
          views: a?.gameVersion === "1.5" ? [customB] : [customA],
          warning: null,
        });
      }
      if (cmd === "get_last_selected_form_view") {
        return Promise.resolve({
          selected:
            a?.gameVersion === "1.5"
              ? { origin: "custom", id: "custom-b" }
              : { origin: "custom", id: "custom-a" },
          warning: null,
        });
      }
      if (cmd === "set_last_selected_form_view") return Promise.resolve(undefined);
      return Promise.reject(new Error(`unexpected invoke: ${cmd}`));
    });

    const catalog = makeCatalog();
    const { result, rerender } = renderHook(
      (props: { gameVersion: string }) =>
        useFormViews({
          projectId: "proj1",
          gameVersion: props.gameVersion,
          catalog,
          pane: null,
          selectedDef: { defType: "ThingDef", ordinal: 0 },
        }),
      { initialProps: { gameVersion: "1.6" } },
    );

    await waitFor(() => expect(result.current.selectedView.id).toBe("custom-a"));

    rerender({ gameVersion: "1.5" });

    // Must resolve to version 1.5's OWN real persisted selection ("custom-b"), never fall back
    // to Default/recommended along the way by wrongly treating "custom-a" (version 1.6's
    // selection, never valid under 1.5) as a confirmed-gone reference for THIS scope.
    await waitFor(() => expect(result.current.selectedView.id).toBe("custom-b"));

    // Critically: no fallback to Default was ever persisted into version 1.5's preference scope.
    expect(invokeMock).not.toHaveBeenCalledWith(
      "set_last_selected_form_view",
      expect.objectContaining({ gameVersion: "1.5", origin: "default" }),
    );

    // Switching back to 1.6 still respects its own real selection too (nothing was clobbered).
    rerender({ gameVersion: "1.6" });
    await waitFor(() => expect(result.current.selectedView.id).toBe("custom-a"));
    expect(invokeMock).not.toHaveBeenCalledWith(
      "set_last_selected_form_view",
      expect.objectContaining({ gameVersion: "1.6", origin: "default" }),
    );
  });

  it("does not let a slow initial preference fetch revert a manual selection made while it was still in flight", async () => {
    const catalog = makeCatalog();
    let resolveGetLastSelected!: (value: {
      selected: { origin: string; id: string } | null;
      warning: null;
    }) => void;
    const getLastSelectedPromise = new Promise<{
      selected: { origin: string; id: string } | null;
      warning: null;
    }>((resolve) => {
      resolveGetLastSelected = resolve;
    });

    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "list_custom_form_views") return Promise.resolve({ views: [], warning: null });
      if (cmd === "get_last_selected_form_view") return getLastSelectedPromise;
      if (cmd === "set_last_selected_form_view") return Promise.resolve(undefined);
      return Promise.reject(new Error(`unexpected invoke: ${cmd}`));
    });

    const { result } = renderHook(() =>
      useFormViews({
        projectId: "proj1",
        gameVersion: "1.6",
        catalog,
        pane: null,
        selectedDef: { defType: "ThingDef", ordinal: 0 },
      }),
    );

    // Before the initial fetch resolves, the user explicitly picks Default (overriding whatever
    // the eventual persisted preference turns out to be). Deliberately NOT wrapped in `act()`
    // and immediately followed (before any `await`) by resolving the stale fetch: this races the
    // fetch's microtask-queued `.then` against React's own effect-cleanup scheduling, which is
    // exactly the real-world ordering the fix guards against (see `selectionGeneration`) -- a
    // synchronous `act()`-wrapped repro would let React's effect cleanup run first and mask the
    // bug regardless of whether the generation guard exists. React logs an "not wrapped in
    // act(...)" warning for this on purpose; it's suppressed below since it's expected, not a
    // real problem with the test.
    const consoleErrorSpy = vi.spyOn(console, "error").mockImplementation(() => undefined);
    try {
      result.current.selectView({ origin: "default", id: "default" });
      // The slow, now-stale fetch resolves right away, reporting a DIFFERENT persisted selection
      // ("weapon") that predates the manual choice above.
      resolveGetLastSelected({ selected: { origin: "schema", id: "weapon" }, warning: null });

      await waitFor(() => expect(result.current.selectedView.origin).toBe("default"));
      // The manual selection must still be in effect -- not silently reverted to "weapon".
      expect(result.current.selectedView.id).toBe("default");
    } finally {
      consoleErrorSpy.mockRestore();
    }
  });

  it("falls back to Default when the persisted selection references a deleted custom view, and re-persists the fallback", async () => {
    mockInvoke({
      get_last_selected_form_view: () => ({
        selected: { origin: "custom", id: "deleted-view" },
        warning: null,
      }),
    });
    // No `formViews` on this Def type at all, so the only possible fallback is Default.
    const catalog: SchemaCatalog = {
      formatVersion: 1,
      packs: [],
      defTypes: {
        RecipeDef: {
          inherits: [],
          abstractType: false,
          fieldOrder: ["defName"],
          fields: {
            defName: { type: { kind: "string" }, required: true, examples: [], repeatable: false, xml: "element", flags: false },
          },
        },
      },
      objectTypes: {},
    };

    const { result } = renderHook(() =>
      useFormViews({
        projectId: "proj1",
        gameVersion: "1.6",
        catalog,
        pane: null,
        selectedDef: { defType: "RecipeDef", ordinal: 0 },
      }),
    );

    await waitFor(() => expect(result.current.selectedView.origin).toBe("default"));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        "set_last_selected_form_view",
        expect.objectContaining({ origin: "default", id: "default" }),
      ),
    );
  });

  it("does NOT overwrite a persisted custom-view selection when the custom-view list fails to load (transient/corrupt store error, not confirmed deletion)", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "list_custom_form_views") {
        // `useCustomFormViews` reports `loading: false` + an empty `views` array after a
        // failed fetch -- the exact same shape as "there really are zero custom views". The
        // reconciliation effect must tell these apart via `customViewsError`, not treat this
        // as proof the persisted "custom-1" reference is gone.
        return Promise.reject(new Error("store read failed"));
      }
      if (cmd === "get_last_selected_form_view") {
        return Promise.resolve({
          selected: { origin: "custom", id: "custom-1" },
          warning: null,
        });
      }
      if (cmd === "set_last_selected_form_view") return Promise.resolve(undefined);
      return Promise.reject(new Error(`unexpected invoke: ${cmd}`));
    });
    const catalog = makeCatalog();

    const { result } = renderHook(() =>
      useFormViews({
        projectId: "proj1",
        gameVersion: "1.6",
        catalog,
        pane: null,
        selectedDef: { defType: "ThingDef", ordinal: 0 },
      }),
    );

    await waitFor(() => expect(result.current.customViewsError).not.toBeNull());
    // Give any (incorrect) reconciliation effect a chance to have fired.
    await new Promise((resolve) => setTimeout(resolve, 0));

    // `resolveSelectedFormView` still has to resolve *something* to render right now (it falls
    // back to the recommended "weapon" schema view since the persisted "custom-1" reference
    // doesn't currently appear in `availableViews` -- the list is empty only because of the
    // error, not because the view was actually deleted). That transient visual fallback is
    // expected and harmless. What must NOT happen is the reconciliation effect treating this as
    // a *confirmed* fallback and persisting over the real stored preference on disk.
    expect(result.current.selectedView.id).toBe("weapon");
    expect(invokeMock).not.toHaveBeenCalledWith(
      "set_last_selected_form_view",
      expect.anything(),
    );
  });

  it("selectView persists a clean selection and clears any override", async () => {
    mockInvoke();
    const catalog = makeCatalog();
    const { result } = renderHook(() =>
      useFormViews({
        projectId: "proj1",
        gameVersion: "1.6",
        catalog,
        pane: null,
        selectedDef: { defType: "ThingDef", ordinal: 0 },
      }),
    );
    await waitFor(() => expect(result.current.selectedView.id).toBe("weapon"));

    act(() => {
      result.current.setOverrideHiddenFieldIds(new Set(["graphicData"]));
    });
    await waitFor(() => expect(result.current.hasDirtyOverride).toBe(true));

    act(() => {
      result.current.selectView({ origin: "default", id: "default" });
    });

    await waitFor(() => expect(result.current.selectedView.origin).toBe("default"));
    expect(result.current.override).toBeNull();
    expect(invokeMock).toHaveBeenCalledWith("set_last_selected_form_view", {
      projectId: "proj1",
      gameVersion: "1.6",
      defType: "ThingDef",
      origin: "default",
      id: "default",
    });
  });

  it("setOverrideHiddenFieldIds/resetOverride drive the dirty-override indicator contract", async () => {
    mockInvoke();
    const catalog = makeCatalog();
    const { result } = renderHook(() =>
      useFormViews({
        projectId: "proj1",
        gameVersion: "1.6",
        catalog,
        pane: null,
        selectedDef: { defType: "ThingDef", ordinal: 0 },
      }),
    );
    await waitFor(() => expect(result.current.selectedView.id).toBe("weapon"));
    expect(result.current.hasDirtyOverride).toBe(false);

    // Identical to the selected view's own hidden set -> not dirty.
    act(() => {
      result.current.setOverrideHiddenFieldIds(new Set(["apparel", "plant"]));
    });
    await waitFor(() => expect(result.current.override).not.toBeNull());
    expect(result.current.hasDirtyOverride).toBe(false);

    // A genuinely different set -> dirty, with an accurate hidden count.
    act(() => {
      result.current.setOverrideHiddenFieldIds(new Set(["apparel", "plant", "graphicData"]));
    });
    await waitFor(() => expect(result.current.hasDirtyOverride).toBe(true));
    expect(result.current.hiddenCount).toBe(3);

    act(() => {
      result.current.resetOverride();
    });
    await waitFor(() => expect(result.current.override).toBeNull());
    expect(result.current.hasDirtyOverride).toBe(false);
    expect(result.current.hiddenCount).toBe(2); // back to the selected view's own hidden set
  });

  it("keeps override/selection state isolated across two Defs opened in the same pane (by ordinal)", async () => {
    mockInvoke();
    const catalog = makeCatalog();
    const { result, rerender } = renderHook(
      (props: { ordinal: number }) =>
        useFormViews({
          projectId: "proj1",
          gameVersion: "1.6",
          catalog,
          pane: null,
          selectedDef: { defType: "ThingDef", ordinal: props.ordinal },
        }),
      { initialProps: { ordinal: 0 } },
    );
    await waitFor(() => expect(result.current.selectedView.id).toBe("weapon"));

    act(() => {
      result.current.setOverrideHiddenFieldIds(new Set(["graphicData"]));
    });
    await waitFor(() => expect(result.current.hasDirtyOverride).toBe(true));

    // Switch to a second Def of the same type in the same file/pane.
    rerender({ ordinal: 1 });
    await waitFor(() => expect(result.current.selectedView.id).toBe("weapon"));
    expect(result.current.override).toBeNull();

    // Switching back to the first Def resurfaces its own stashed override.
    rerender({ ordinal: 0 });
    await waitFor(() => expect(result.current.hasDirtyOverride).toBe(true));
    expect([...(result.current.override?.hiddenFieldIds ?? [])]).toEqual(["graphicData"]);
  });

  it("two independent hook instances (two panes) never share override state", async () => {
    mockInvoke();
    const catalog = makeCatalog();
    const argsFor = (ordinal: number) => ({
      projectId: "proj1",
      gameVersion: "1.6",
      catalog,
      pane: null,
      selectedDef: { defType: "ThingDef", ordinal },
    });

    const paneA = renderHook(() => useFormViews(argsFor(0)));
    const paneB = renderHook(() => useFormViews(argsFor(0)));

    await waitFor(() => expect(paneA.result.current.selectedView.id).toBe("weapon"));
    await waitFor(() => expect(paneB.result.current.selectedView.id).toBe("weapon"));

    act(() => {
      paneA.result.current.setOverrideHiddenFieldIds(new Set(["graphicData"]));
    });
    await waitFor(() => expect(paneA.result.current.hasDirtyOverride).toBe(true));

    expect(paneB.result.current.override).toBeNull();
    expect(paneB.result.current.hasDirtyOverride).toBe(false);
  });

  it("deleting the currently-selected custom view falls back to Default", async () => {
    const customView: CustomFormView = {
      id: "custom-1",
      target: { gameVersion: "1.6", defType: "ThingDef" },
      name: "My view",
      description: null,
      hiddenFieldIds: ["graphicData"],
      baseSchemaView: null,
      createdAt: "2026-01-01T00:00:00Z",
      updatedAt: "2026-01-01T00:00:00Z",
    };
    let deleted = false;
    mockInvoke({
      list_custom_form_views: () => ({ views: deleted ? [] : [customView], warning: null }),
      delete_custom_form_view: () => {
        deleted = true;
        return { deletedId: "custom-1" };
      },
    });
    const catalog = makeCatalog();
    const { result } = renderHook(() =>
      useFormViews({
        projectId: "proj1",
        gameVersion: "1.6",
        catalog,
        pane: null,
        selectedDef: { defType: "ThingDef", ordinal: 0 },
      }),
    );
    await waitFor(() => expect(result.current.customViews).toHaveLength(1));

    act(() => {
      result.current.selectView({ origin: "custom", id: "custom-1" });
    });
    await waitFor(() => expect(result.current.selectedView.id).toBe("custom-1"));

    await act(async () => {
      await result.current.deleteCustomView("custom-1");
    });

    await waitFor(() => expect(result.current.selectedView.origin).toBe("default"));
  });

  it("end-to-end: selecting a real resolved schema view actually changes which fields buildFormFieldModels renders", async () => {
    mockInvoke();
    const catalog = makeCatalog();
    const { result } = renderHook(() =>
      useFormViews({
        projectId: "proj1",
        gameVersion: "1.6",
        catalog,
        pane: null,
        selectedDef: { defType: "ThingDef", ordinal: 0 },
      }),
    );
    await waitFor(() => expect(result.current.selectedView.id).toBe("weapon"));

    const def: DefEditorView = {
      nodeId: 1,
      defType: "ThingDef",
      defName: "Test",
      label: null,
      parentName: null,
      line: null,
      column: null,
      attributes: [],
      children: [],
    };

    // "weapon" hides apparel/plant -- the catalog's own real, resolved schema view data (issue
    // 03), flowing through `useFormViews` (issue 06) into `buildFormFieldModels` (issue 05).
    const hiddenModels = buildFormFieldModels(
      def,
      catalog.defTypes.ThingDef,
      catalog,
      result.current.visibleTopLevelFieldIds,
    );
    expect(hiddenModels.map((m) => m.key).sort()).toEqual(["defName", "graphicData"]);

    // Selecting Default restores the full field set without touching the XML/session at all.
    act(() => {
      result.current.selectDefaultView();
    });
    await waitFor(() => expect(result.current.selectedView.origin).toBe("default"));

    const fullModels = buildFormFieldModels(
      def,
      catalog.defTypes.ThingDef,
      catalog,
      result.current.visibleTopLevelFieldIds,
    );
    expect(fullModels.map((m) => m.key).sort()).toEqual([
      "apparel",
      "defName",
      "graphicData",
      "plant",
    ]);
  });

  it("end-to-end through the real form controller: useXmlFormController's own snapshot.fields reflects the selected view's visibility, not just the pure buildFormFieldModels call", async () => {
    mockInvoke();
    const catalog = makeCatalog();

    function useCombined() {
      const formViews = useFormViews({
        projectId: "proj1",
        gameVersion: "1.6",
        catalog,
        pane: null,
        selectedDef: { defType: "ThingDef", ordinal: 0 },
      });
      const snapshot: XmlEditorSnapshot = {
        rawXml: "<Defs><ThingDef><defName>Test</defName></ThingDef></Defs>",
        parsed: {
          nodeCount: 1,
          rootElement: "Defs",
          profile: "defs",
          about: null,
          defs: [
            {
              nodeId: 1,
              defType: "ThingDef",
              defName: "Test",
              label: null,
              parentName: null,
              line: null,
              column: null,
              attributes: [],
              children: [],
            },
          ],
        },
        parseDiagnostics: [],
        validationDiagnostics: [],
        selectedDefNodeId: 1,
      };
      const formApi = useXmlFormController({
        snapshot,
        catalog,
        selectedDefNodeId: 1,
        commitEdits: async () => "",
        clearPreview: () => undefined,
        visibleTopLevelFieldIds: formViews.visibleTopLevelFieldIds,
      });
      return { formViews, formApi };
    }

    const { result } = renderHook(() => useCombined());
    await waitFor(() => expect(result.current.formViews.selectedView.id).toBe("weapon"));

    // The real form controller's own field snapshot -- exactly what `XmlFormEditor` renders
    // from -- excludes the fields "weapon" hides, proving the wiring through
    // `useXmlFormController` itself rather than only through the pure resolver/descriptor
    // functions in isolation.
    await waitFor(() => {
      const keys = (result.current.formApi.snapshot?.fields ?? []).map((f) => f.model.key);
      expect(keys.sort()).toEqual(["defName", "graphicData"]);
    });

    act(() => {
      result.current.formViews.selectDefaultView();
    });
    await waitFor(() => expect(result.current.formViews.selectedView.origin).toBe("default"));

    await waitFor(() => {
      const keys = (result.current.formApi.snapshot?.fields ?? []).map((f) => f.model.key);
      expect(keys.sort()).toEqual(["apparel", "defName", "graphicData", "plant"]);
    });
  });

  it("saveOverrideAsCustomView rejects with a structured diagnostic code when there is no unsaved override", async () => {
    // Known, enumerable precondition this hook detects itself (no dirty `FieldVisibilityOverride`
    // to persist) -- must carry a `code` the shared renderer can translate (see
    // `src/i18n/diagnostics.ts`), not only an English `Error.message` that bypasses localization
    // entirely. Mirrors `useCustomFormViews.test.ts`'s equivalent assertion for the same bug class.
    mockInvoke();
    const catalog = makeCatalog();

    const { result } = renderHook(() =>
      useFormViews({
        projectId: "proj1",
        gameVersion: "1.6",
        catalog,
        pane: null,
        selectedDef: { defType: "ThingDef", ordinal: 0 },
      }),
    );
    await waitFor(() => expect(result.current.selectedView.id).toBe("weapon"));
    expect(result.current.hasDirtyOverride).toBe(false);

    await expect(result.current.saveOverrideAsCustomView("My saved view")).rejects.toMatchObject({
      code: "form_view_no_unsaved_changes",
    });
  });

  describe("stale CRUD-then-select/persist completions never affect a since-abandoned scope", () => {
    // Shared shape: scope A (game version 1.6) starts a slow async operation with a chained
    // side effect (auto-selecting a freshly created/duplicated view, auto-selecting Default
    // after deleting the current view, or persisting a selection), the scope switches to B
    // (1.5) while that operation is still in flight, and only then does A's operation resolve.
    // In every case, A's stale completion's chained side effect must never touch scope B's
    // now-current selection/override/warning state.
    function mockInvokeForScopeRace(overrides: Record<string, (args: unknown) => unknown> = {}) {
      invokeMock.mockImplementation((cmd: string, args?: unknown) => {
        if (cmd in overrides) return Promise.resolve(overrides[cmd](args));
        if (cmd === "list_custom_form_views") return Promise.resolve({ views: [], warning: null });
        if (cmd === "get_last_selected_form_view") return Promise.resolve({ selected: null, warning: null });
        if (cmd === "set_last_selected_form_view") return Promise.resolve(undefined);
        return Promise.reject(new Error(`unexpected invoke: ${cmd}`));
      });
    }

    it("saveOverrideAsCustomView's chained selectView", async () => {
      // Scope B has its OWN genuine, distinct persisted custom-view selection ("custom-b").
      // This matters for the test to actually be able to detect corruption: if B instead had no
      // custom views/preference at all, a stale completion that corrupted `active` to point at
      // scope A's newly created view would still innocuously *resolve* back to B's own
      // recommended schema view (since the bogus custom id wouldn't match anything in B's empty
      // list either) -- making the bug invisible to an assertion on the resolved `selectedView`
      // alone. Giving B a real custom selection means any corruption is observable: it would
      // stop resolving to "custom-b".
      const customB: CustomFormView = {
        id: "custom-b",
        target: { gameVersion: "1.5", defType: "ThingDef" },
        name: "Custom B",
        description: null,
        hiddenFieldIds: ["plant"],
        baseSchemaView: null,
        createdAt: "2026-01-01T00:00:00Z",
        updatedAt: "2026-01-01T00:00:00Z",
      };
      let resolveCreate!: (view: CustomFormView) => void;
      const createPromise = new Promise<CustomFormView>((resolve) => {
        resolveCreate = resolve;
      });
      mockInvokeForScopeRace({
        create_custom_form_view: () => createPromise,
        list_custom_form_views: (args) => {
          const a = args as { gameVersion: string };
          return { views: a.gameVersion === "1.5" ? [customB] : [], warning: null };
        },
        get_last_selected_form_view: (args) => {
          const a = args as { gameVersion: string };
          return {
            selected: a.gameVersion === "1.5" ? { origin: "custom", id: "custom-b" } : null,
            warning: null,
          };
        },
      });
      const catalog = makeCatalog();

      const { result, rerender } = renderHook(
        (props: { gameVersion: string }) =>
          useFormViews({
            projectId: "proj1",
            gameVersion: props.gameVersion,
            catalog,
            pane: null,
            selectedDef: { defType: "ThingDef", ordinal: 0 },
          }),
        { initialProps: { gameVersion: "1.6" } },
      );
      await waitFor(() => expect(result.current.selectedView.id).toBe("weapon"));

      act(() => {
        result.current.setOverrideHiddenFieldIds(new Set(["graphicData"]));
      });
      await waitFor(() => expect(result.current.hasDirtyOverride).toBe(true));

      let saveCall!: Promise<CustomFormView>;
      act(() => {
        saveCall = result.current.saveOverrideAsCustomView("My saved view");
      });

      // Switch to scope B (1.5) while A's create is still pending. B resolves to its own real,
      // distinct persisted custom-view selection.
      act(() => {
        rerender({ gameVersion: "1.5" });
      });
      await waitFor(() => expect(result.current.selectedView.id).toBe("custom-b"));
      expect(result.current.hasDirtyOverride).toBe(false);

      await act(async () => {
        resolveCreate({
          id: "created-under-a",
          target: { gameVersion: "1.6", defType: "ThingDef" },
          name: "My saved view",
          description: null,
          hiddenFieldIds: ["graphicData"],
          baseSchemaView: null,
          createdAt: "2026-01-01T00:00:00Z",
          updatedAt: "2026-01-01T00:00:00Z",
        });
        await saveCall.catch(() => undefined);
      });

      // Scope B's selection must remain its own "custom-b" -- not switched to the view created
      // under scope A, and no override resurrected from A's save.
      expect(result.current.selectedView.origin).toBe("custom");
      expect(result.current.selectedView.id).toBe("custom-b");
      expect(result.current.hasDirtyOverride).toBe(false);
      expect(result.current.override).toBeNull();
    });

    it("deleteCustomView's chained selectDefaultView", async () => {
      const customView: CustomFormView = {
        id: "custom-a",
        target: { gameVersion: "1.6", defType: "ThingDef" },
        name: "Custom A",
        description: null,
        hiddenFieldIds: ["graphicData"],
        baseSchemaView: null,
        createdAt: "2026-01-01T00:00:00Z",
        updatedAt: "2026-01-01T00:00:00Z",
      };
      let resolveDelete!: (result: { deletedId: string }) => void;
      const deletePromise = new Promise<{ deletedId: string }>((resolve) => {
        resolveDelete = resolve;
      });
      mockInvokeForScopeRace({
        list_custom_form_views: (args) => {
          const a = args as { gameVersion: string };
          return { views: a.gameVersion === "1.6" ? [customView] : [], warning: null };
        },
        get_last_selected_form_view: (args) => {
          const a = args as { gameVersion: string };
          return {
            selected: a.gameVersion === "1.6" ? { origin: "custom", id: "custom-a" } : null,
            warning: null,
          };
        },
        delete_custom_form_view: () => deletePromise,
      });
      const catalog = makeCatalog();

      const { result, rerender } = renderHook(
        (props: { gameVersion: string }) =>
          useFormViews({
            projectId: "proj1",
            gameVersion: props.gameVersion,
            catalog,
            pane: null,
            selectedDef: { defType: "ThingDef", ordinal: 0 },
          }),
        { initialProps: { gameVersion: "1.6" } },
      );
      await waitFor(() => expect(result.current.selectedView.id).toBe("custom-a"));

      let deleteCall!: Promise<void>;
      act(() => {
        deleteCall = result.current.deleteCustomView("custom-a");
      });

      // Switch to scope B (1.5) while A's delete is still pending. B has no custom views and no
      // persisted preference, so it resolves to its own recommended view.
      act(() => {
        rerender({ gameVersion: "1.5" });
      });
      await waitFor(() => expect(result.current.selectedView.id).toBe("weapon"));

      await act(async () => {
        resolveDelete({ deletedId: "custom-a" });
        await deleteCall.catch(() => undefined);
      });

      // Scope B's selection must remain its own recommended view -- deleting scope A's
      // then-current custom view must never force scope B to Default.
      expect(result.current.selectedView.id).toBe("weapon");
    });

    it("selection-persist warning does not leak across a scope switch", async () => {
      let rejectPersistA!: (e: Error) => void;
      const persistAPromise = new Promise<void>((_resolve, reject) => {
        rejectPersistA = reject;
      });
      invokeMock.mockImplementation((cmd: string, args?: unknown) => {
        if (cmd === "list_custom_form_views") return Promise.resolve({ views: [], warning: null });
        if (cmd === "get_last_selected_form_view")
          return Promise.resolve({ selected: null, warning: null });
        if (cmd === "set_last_selected_form_view") {
          const a = args as { gameVersion: string };
          if (a.gameVersion === "1.6") return persistAPromise;
          return Promise.resolve(undefined); // Scope B persists successfully.
        }
        return Promise.reject(new Error(`unexpected invoke: ${cmd}`));
      });
      const catalog = makeCatalog();

      const { result, rerender } = renderHook(
        (props: { gameVersion: string }) =>
          useFormViews({
            projectId: "proj1",
            gameVersion: props.gameVersion,
            catalog,
            pane: null,
            selectedDef: { defType: "ThingDef", ordinal: 0 },
          }),
        { initialProps: { gameVersion: "1.6" } },
      );
      await waitFor(() => expect(result.current.selectedView.id).toBe("weapon"));

      // Selecting Default under scope A kicks off a slow, never-yet-settled persist call.
      act(() => {
        result.current.selectView({ origin: "default", id: "default" });
      });
      expect(result.current.persistWarning).toBeNull();

      // Switch to scope B while A's persist is still pending, and give it a real clean
      // selection of its own.
      act(() => {
        rerender({ gameVersion: "1.5" });
      });
      await waitFor(() => expect(result.current.selectedView.id).toBe("weapon"));
      act(() => {
        result.current.selectView({ origin: "default", id: "default" });
      });
      await waitFor(() => expect(result.current.selectedView.origin).toBe("default"));
      expect(result.current.persistWarning).toBeNull();

      // Now scope A's slow persist finally rejects.
      await act(async () => {
        rejectPersistA(new Error("disk full"));
        await new Promise((resolve) => setTimeout(resolve, 0));
      });

      // Scope B never had a failed persist -- A's stale rejection must not display a warning
      // against B's current (successfully persisted) state.
      expect(result.current.persistWarning).toBeNull();
    });

    it("a stale, same-scope persist rejection does not override a newer successful persist's clean state", async () => {
      // Both persist attempts below happen in the SAME scope (game version never changes) --
      // this is exactly what `scopeGenerationRef` alone cannot catch (it only tells scopes
      // apart from each other), and why a separate per-scope "latest selection attempt" counter
      // is needed too.
      let rejectFirstPersist!: (e: Error) => void;
      const firstPersistPromise = new Promise<void>((_resolve, reject) => {
        rejectFirstPersist = reject;
      });
      let persistCallCount = 0;
      invokeMock.mockImplementation((cmd: string) => {
        if (cmd === "list_custom_form_views") return Promise.resolve({ views: [], warning: null });
        if (cmd === "get_last_selected_form_view")
          return Promise.resolve({ selected: null, warning: null });
        if (cmd === "set_last_selected_form_view") {
          persistCallCount += 1;
          // The FIRST selectView's persist call is the slow one; the SECOND resolves promptly.
          return persistCallCount === 1 ? firstPersistPromise : Promise.resolve(undefined);
        }
        return Promise.reject(new Error(`unexpected invoke: ${cmd}`));
      });
      const catalog = makeCatalog();

      const { result } = renderHook(() =>
        useFormViews({
          projectId: "proj1",
          gameVersion: "1.6",
          catalog,
          pane: null,
          selectedDef: { defType: "ThingDef", ordinal: 0 },
        }),
      );
      await waitFor(() => expect(result.current.selectedView.id).toBe("weapon"));

      // Select view A -- its persist call is the slow one and stays pending.
      act(() => {
        result.current.selectView({ origin: "schema", id: "weapon" });
      });

      // Promptly select Default instead -- a NEWER attempt in the SAME scope, whose persist
      // resolves successfully right away.
      await act(async () => {
        result.current.selectView({ origin: "default", id: "default" });
        await Promise.resolve();
      });
      await waitFor(() => expect(result.current.selectedView.origin).toBe("default"));
      expect(result.current.persistWarning).toBeNull();

      // NOW the first (superseded) persist attempt finally rejects.
      await act(async () => {
        rejectFirstPersist(new Error("disk full"));
        await new Promise((resolve) => setTimeout(resolve, 0));
      });

      // The current (Default) selection was saved successfully -- an older, already-superseded
      // attempt's failure must not retroactively display a warning against it.
      expect(result.current.selectedView.origin).toBe("default");
      expect(result.current.persistWarning).toBeNull();
    });
  });
});
