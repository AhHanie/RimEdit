import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FormViewFieldChecklist } from "./FormViewFieldChecklist";
import type { UseFormViewsResult } from "../../hooks/useFormViews";
import type { ResolvedFormView } from "../../types/resolvedFormView";
import type { DefEditorView } from "../../../xml-editor/types/xmlDocument";
import type { SchemaCatalog } from "../../../schema-catalog";

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

function makeCatalog(): SchemaCatalog {
  return {
    formatVersion: 1,
    packs: [],
    objectTypes: {
      GraphicData: { fieldOrder: [], fields: {} },
    },
    defTypes: {
      ThingDef: {
        inherits: [],
        abstractType: false,
        fieldOrder: ["defName", "graphicData", "apparel"],
        fields: {
          defName: {
            type: { kind: "string" },
            required: false,
            examples: [],
            repeatable: false,
            xml: "element",
            flags: false,
          },
          graphicData: {
            type: { kind: "object", schemaRef: "GraphicData" },
            required: false,
            examples: [],
            repeatable: false,
            xml: "object",
            flags: false,
          },
          apparel: {
            type: { kind: "object", schemaRef: "GraphicData" },
            required: false,
            examples: [],
            repeatable: false,
            xml: "object",
            flags: false,
          },
        },
      },
    },
  };
}

function makeAttributeAndListCatalog(): SchemaCatalog {
  return {
    formatVersion: 1,
    packs: [],
    objectTypes: {},
    defTypes: {
      ThingDef: {
        inherits: [],
        abstractType: false,
        fieldOrder: ["Abstract", "tags"],
        fields: {
          Abstract: {
            type: { kind: "boolean" },
            required: false,
            examples: [],
            repeatable: false,
            xml: "attribute",
            flags: false,
          },
          tags: {
            type: { kind: "string" },
            required: false,
            examples: [],
            repeatable: false,
            xml: "listOfLi",
            flags: false,
          },
        },
      },
    },
  };
}

function view(overrides: Partial<ResolvedFormView> = {}): ResolvedFormView {
  return {
    id: "default",
    targetDefType: "ThingDef",
    label: "Default View",
    order: 0,
    origin: "default",
    hiddenFieldIds: [],
    recommended: false,
    ...overrides,
  };
}

function makeController(overrides: Partial<UseFormViewsResult> = {}): UseFormViewsResult {
  const defaultView = view();
  return {
    applicable: true,
    availableViews: [defaultView],
    selectedView: defaultView,
    effectiveHidden: new Set(),
    hiddenCount: 0,
    visibleTopLevelFieldIds: null,
    override: null,
    hasDirtyOverride: false,
    selectView: vi.fn(),
    selectDefaultView: vi.fn(),
    resetOverride: vi.fn(),
    setOverrideHiddenFieldIds: vi.fn(),
    saveOverrideAsCustomView: vi.fn().mockResolvedValue(undefined),
    duplicateAsCustomView: vi.fn(),
    createCustomView: vi.fn(),
    renameCustomView: vi.fn(),
    updateCustomView: vi.fn().mockResolvedValue(undefined),
    deleteCustomView: vi.fn(),
    getScopeKey: vi.fn().mockReturnValue("test-scope"),
    customViews: [],
    customViewsLoading: false,
    customViewsWarning: null,
    customViewsError: null,
    reloadCustomViews: vi.fn().mockResolvedValue(undefined),
    resetCustomViewStore: vi.fn().mockResolvedValue({ backupPath: null }),
    persistWarning: null,
    ...overrides,
  };
}

describe("FormViewFieldChecklist", () => {
  it("renders every effective top-level schema field as a checkbox row in schema order", () => {
    const controller = makeController();
    render(
      <FormViewFieldChecklist
        controller={controller}
        def={makeDef()}
        defSchema={makeCatalog().defTypes.ThingDef}
        catalog={makeCatalog()}
      />,
    );

    const checkboxes = screen.getAllByRole("checkbox");
    expect(checkboxes).toHaveLength(3);
    expect(checkboxes.every((cb) => (cb as HTMLInputElement).checked)).toBe(true);
  });

  it("gives an object-root field a Section badge", () => {
    const controller = makeController();
    render(
      <FormViewFieldChecklist
        controller={controller}
        def={makeDef()}
        defSchema={makeCatalog().defTypes.ThingDef}
        catalog={makeCatalog()}
      />,
    );
    expect(screen.getAllByText("Section")).toHaveLength(2); // graphicData, apparel
  });

  it("toggling a checkbox calls setOverrideHiddenFieldIds with the field added to the hidden set", async () => {
    const controller = makeController();
    render(
      <FormViewFieldChecklist
        controller={controller}
        def={makeDef()}
        defSchema={makeCatalog().defTypes.ThingDef}
        catalog={makeCatalog()}
      />,
    );

    await userEvent.click(screen.getByLabelText("defName"));
    expect(controller.setOverrideHiddenFieldIds).toHaveBeenCalledWith(new Set(["defName"]));
  });

  it("unchecking an already-hidden field removes it from the override", async () => {
    const controller = makeController({ effectiveHidden: new Set(["defName"]) });
    render(
      <FormViewFieldChecklist
        controller={controller}
        def={makeDef()}
        defSchema={makeCatalog().defTypes.ThingDef}
        catalog={makeCatalog()}
      />,
    );

    const checkbox = screen.getByLabelText("defName") as HTMLInputElement;
    expect(checkbox.checked).toBe(false);
    await userEvent.click(checkbox);
    expect(controller.setOverrideHiddenFieldIds).toHaveBeenCalledWith(new Set());
  });

  it("Show all clears the hidden set and Hide all hides every known field", async () => {
    const controller = makeController({ effectiveHidden: new Set(["defName"]) });
    render(
      <FormViewFieldChecklist
        controller={controller}
        def={makeDef()}
        defSchema={makeCatalog().defTypes.ThingDef}
        catalog={makeCatalog()}
      />,
    );

    await userEvent.click(screen.getByText("Show all"));
    expect(controller.setOverrideHiddenFieldIds).toHaveBeenCalledWith(new Set());

    await userEvent.click(screen.getByText("Hide all"));
    expect(controller.setOverrideHiddenFieldIds).toHaveBeenCalledWith(
      new Set(["defName", "graphicData", "apparel"]),
    );
  });

  it("Reset to selected view calls resetOverride and is disabled without a dirty override", async () => {
    const controller = makeController({ hasDirtyOverride: false });
    render(
      <FormViewFieldChecklist
        controller={controller}
        def={makeDef()}
        defSchema={makeCatalog().defTypes.ThingDef}
        catalog={makeCatalog()}
      />,
    );
    expect((screen.getByText("Reset to selected view") as HTMLButtonElement).disabled).toBe(true);

    const dirtyController = makeController({ hasDirtyOverride: true });
    render(
      <FormViewFieldChecklist
        controller={dirtyController}
        def={makeDef()}
        defSchema={makeCatalog().defTypes.ThingDef}
        catalog={makeCatalog()}
      />,
    );
    const resetButtons = screen.getAllByText("Reset to selected view");
    await userEvent.click(resetButtons[resetButtons.length - 1]);
    expect(dirtyController.resetOverride).toHaveBeenCalledTimes(1);
  });

  it("filters rows by label text", async () => {
    const controller = makeController();
    render(
      <FormViewFieldChecklist
        controller={controller}
        def={makeDef()}
        defSchema={makeCatalog().defTypes.ThingDef}
        catalog={makeCatalog()}
      />,
    );

    await userEvent.type(screen.getByLabelText("Filter fields"), "apparel");
    expect(screen.getAllByRole("checkbox")).toHaveLength(1);
    expect(screen.getByLabelText("apparel")).toBeTruthy();
  });

  it("shows a warning banner when every known field is hidden", () => {
    const controller = makeController({
      effectiveHidden: new Set(["defName", "graphicData", "apparel"]),
      hiddenCount: 3,
    });
    render(
      <FormViewFieldChecklist
        controller={controller}
        def={makeDef()}
        defSchema={makeCatalog().defTypes.ThingDef}
        catalog={makeCatalog()}
      />,
    );
    expect(screen.getByRole("status").textContent).toMatch(/All fields are hidden/);
  });

  it("shows an unknown-XML note that Form Views never affects unknown fields", () => {
    const controller = makeController();
    render(
      <FormViewFieldChecklist
        controller={controller}
        def={makeDef()}
        defSchema={makeCatalog().defTypes.ThingDef}
        catalog={makeCatalog()}
      />,
    );
    expect(screen.getByText(/never affected by Form Views/)).toBeTruthy();
  });

  it("offers Save changes (not Save as custom view) for a custom-origin selected view, gated on a dirty override", () => {
    const custom = view({ id: "custom-1", origin: "custom", label: "My view" });
    const controller = makeController({
      selectedView: custom,
      hasDirtyOverride: true,
      override: { hiddenFieldIds: new Set(["apparel"]), isDirty: true },
    });
    render(
      <FormViewFieldChecklist
        controller={controller}
        def={makeDef()}
        defSchema={makeCatalog().defTypes.ThingDef}
        catalog={makeCatalog()}
      />,
    );
    expect((screen.getByText("Save changes") as HTMLButtonElement).disabled).toBe(false);
    expect(screen.queryByText("Save as custom view")).toBeNull();
  });

  it("Save changes updates the custom view's hidden set and clears the override", async () => {
    const custom = view({ id: "custom-1", origin: "custom", label: "My view" });
    const controller = makeController({
      selectedView: custom,
      hasDirtyOverride: true,
      override: { hiddenFieldIds: new Set(["apparel"]), isDirty: true },
    });
    render(
      <FormViewFieldChecklist
        controller={controller}
        def={makeDef()}
        defSchema={makeCatalog().defTypes.ThingDef}
        catalog={makeCatalog()}
      />,
    );

    await userEvent.click(screen.getByText("Save changes"));
    expect(controller.updateCustomView).toHaveBeenCalledWith("custom-1", {
      hiddenFieldIds: ["apparel"],
    });
    expect(controller.resetOverride).toHaveBeenCalledTimes(1);
  });

  it("offers an inline Save as custom view prompt for Default/schema origin, gated on a dirty override", async () => {
    const controller = makeController({
      hasDirtyOverride: true,
      override: { hiddenFieldIds: new Set(["apparel"]), isDirty: true },
    });
    render(
      <FormViewFieldChecklist
        controller={controller}
        def={makeDef()}
        defSchema={makeCatalog().defTypes.ThingDef}
        catalog={makeCatalog()}
      />,
    );
    expect(screen.queryByText("Save changes")).toBeNull();

    await userEvent.click(screen.getByText("Save as custom view"));
    await userEvent.type(screen.getByLabelText("Custom view name"), "My weapon view");
    await userEvent.click(screen.getByText("Save"));

    expect(controller.saveOverrideAsCustomView).toHaveBeenCalledWith("My weapon view");
  });

  it("disables Save as custom view without a dirty override", () => {
    const controller = makeController({ hasDirtyOverride: false });
    render(
      <FormViewFieldChecklist
        controller={controller}
        def={makeDef()}
        defSchema={makeCatalog().defTypes.ThingDef}
        catalog={makeCatalog()}
      />,
    );
    expect((screen.getByText("Save as custom view") as HTMLButtonElement).disabled).toBe(true);
  });

  it("toggling an attribute-shaped field checkbox updates the override", async () => {
    const controller = makeController();
    const catalog = makeAttributeAndListCatalog();
    render(
      <FormViewFieldChecklist
        controller={controller}
        def={makeDef()}
        defSchema={catalog.defTypes.ThingDef}
        catalog={catalog}
      />,
    );

    await userEvent.click(screen.getByLabelText("Abstract"));
    expect(controller.setOverrideHiddenFieldIds).toHaveBeenCalledWith(new Set(["Abstract"]));
  });

  it("toggling a list-root field checkbox updates the override", async () => {
    const controller = makeController();
    const catalog = makeAttributeAndListCatalog();
    render(
      <FormViewFieldChecklist
        controller={controller}
        def={makeDef()}
        defSchema={catalog.defTypes.ThingDef}
        catalog={catalog}
      />,
    );

    await userEvent.click(screen.getByLabelText("tags"));
    expect(controller.setOverrideHiddenFieldIds).toHaveBeenCalledWith(new Set(["tags"]));
  });

  it("preserves orphaned hidden field ids no longer in the current schema when saving changes (finding 2)", async () => {
    const custom = view({
      id: "custom-1",
      origin: "custom",
      label: "My view",
      // "removedField" is no longer part of the current schema/checklist at all.
      hiddenFieldIds: ["removedField", "apparel"],
    });
    const controller = makeController({
      selectedView: custom,
      hasDirtyOverride: true,
      // The user additionally hid "defName" via the checklist; "apparel" stays hidden.
      override: { hiddenFieldIds: new Set(["apparel", "defName"]), isDirty: true },
    });
    render(
      <FormViewFieldChecklist
        controller={controller}
        def={makeDef()}
        defSchema={makeCatalog().defTypes.ThingDef}
        catalog={makeCatalog()}
      />,
    );

    await userEvent.click(screen.getByText("Save changes"));

    expect(controller.updateCustomView).toHaveBeenCalledTimes(1);
    const [viewId, updates] = (controller.updateCustomView as ReturnType<typeof vi.fn>).mock
      .calls[0] as [string, { hiddenFieldIds: string[] }];
    expect(viewId).toBe("custom-1");
    expect([...updates.hiddenFieldIds].sort()).toEqual(["apparel", "defName", "removedField"]);
  });

  it("does not clear the override or surface an error if the scope changes while Save changes is in flight (finding 3)", async () => {
    let resolveUpdate!: () => void;
    const updatePromise = new Promise<void>((resolve) => {
      resolveUpdate = resolve;
    });
    const custom = view({ id: "custom-1", origin: "custom", label: "My view" });
    const getScopeKey = vi.fn().mockReturnValueOnce("scope-A").mockReturnValue("scope-B");
    const controller = makeController({
      selectedView: custom,
      hasDirtyOverride: true,
      override: { hiddenFieldIds: new Set(["apparel"]), isDirty: true },
      updateCustomView: vi.fn().mockReturnValue(updatePromise),
      getScopeKey,
    });
    render(
      <FormViewFieldChecklist
        controller={controller}
        def={makeDef()}
        defSchema={makeCatalog().defTypes.ThingDef}
        catalog={makeCatalog()}
      />,
    );

    await userEvent.click(screen.getByText("Save changes"));
    // Simulate the Def/tab/game-version scope moving on (e.g. the user switched to a
    // different Def in this same pane) while the write was still in flight.
    resolveUpdate();
    await waitFor(() => expect(getScopeKey).toHaveBeenCalledTimes(2));

    // The write itself landed correctly under its original scope, but this stale
    // completion must never clear whatever override now belongs to the active scope.
    expect(controller.resetOverride).not.toHaveBeenCalled();
    expect(screen.queryByText(/error/i)).toBeNull();
  });

  it("does not clear the inline save-as-custom prompt if the scope changes while saving (finding 3)", async () => {
    let resolveSave!: () => void;
    const savePromise = new Promise<void>((resolve) => {
      resolveSave = resolve;
    });
    const getScopeKey = vi.fn().mockReturnValueOnce("scope-A").mockReturnValue("scope-B");
    const controller = makeController({
      hasDirtyOverride: true,
      override: { hiddenFieldIds: new Set(["apparel"]), isDirty: true },
      saveOverrideAsCustomView: vi.fn().mockReturnValue(savePromise),
      getScopeKey,
    });
    render(
      <FormViewFieldChecklist
        controller={controller}
        def={makeDef()}
        defSchema={makeCatalog().defTypes.ThingDef}
        catalog={makeCatalog()}
      />,
    );

    await userEvent.click(screen.getByText("Save as custom view"));
    await userEvent.type(screen.getByLabelText("Custom view name"), "In progress for Def B");
    await userEvent.click(screen.getByText("Save"));

    resolveSave();
    await waitFor(() => expect(getScopeKey).toHaveBeenCalledTimes(2));

    // A stale completion from an abandoned scope must not silently wipe out what the user
    // is now doing in this same panel for a DIFFERENT (current) scope.
    expect((screen.getByLabelText("Custom view name") as HTMLInputElement).value).toBe(
      "In progress for Def B",
    );
  });
});
