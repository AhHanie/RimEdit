import { render, screen, within } from "@testing-library/react";
import { vi, describe, it, expect } from "vitest";
import { XmlFormEditor } from "./XmlFormEditor";
import { XmlEditorContextProvider } from "../../context/XmlEditorContext";
import type { XmlFormApi } from "../../hooks/useXmlFormController";
import type {
  FormControlKind,
  FormFieldPath,
  FormFieldState,
  FormValue,
  FormSectionDefaults,
} from "../../types/editorForm";
import type { XmlEditorSnapshot } from "../../types/editorSession";
import {
  FormFieldStore,
  type StoredFieldState,
} from "../../lib/formFieldStore";
import { makeGraphicDataFormState } from "../../__fixtures__/graphicData";
import type { UseFormViewsResult } from "../../../form-views/hooks/useFormViews";
import type { ResolvedFormView } from "../../../form-views/types/resolvedFormView";

type GraphicDataPreviewProps = {
  projectId?: string;
  texPath: string;
  graphicClass: string;
  maskPath?: string;
};
const capturedPreviewProps: GraphicDataPreviewProps[] = [];

vi.mock("../GraphicDataPreview/GraphicDataPreview", () => ({
  GraphicDataPreview: (props: GraphicDataPreviewProps) => {
    capturedPreviewProps.push({ ...props });
    return null;
  },
}));

function makeSnapshot(): XmlEditorSnapshot {
  return {
    rawXml: "<Defs><ThingDef><defName>Test</defName></ThingDef></Defs>",
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
  };
}

function makeField(
  id: string,
  key: string,
  label: string,
  path: FormFieldPath,
  control: FormControlKind = "text",
  order: number = 0,
): FormFieldState {
  const value: FormValue =
    control === "namedMap"
      ? { kind: "namedMap", entries: [] }
      : control === "list"
        ? { kind: "list", items: [] }
        : { kind: "scalar", value: "" };
  return {
    model: {
      id,
      key,
      label,
      control,
      path,
      fieldPath: key.split("."),
      defNodeId: 1,
      sourceNodeId: null,
      order,
      readonly: false,
      required: false,
      repeatable: false,
      xmlShape: "element",
      examples: [],
      diagnostics: [],
      sectionDefaults: [],
    },
    value,
    initialValue: value,
    dirty: false,
    touched: false,
    focused: false,
    pending: false,
    error: null,
    validationErrors: [],
    clearRequested: false,
  };
}

function makeStore(fields: FormFieldState[]): FormFieldStore {
  const store = new FormFieldStore();
  const models = fields.map((f) => f.model);
  const map = new Map<string, StoredFieldState>(
    fields.map((f) => [
      f.model.id,
      {
        model: f.model,
        value: f.value,
        initialValue: f.initialValue,
        dirty: f.dirty,
        touched: f.touched,
        focused: f.focused,
        pending: f.pending,
        error: f.error,
        cachedValidationErrors: f.validationErrors,
        clearRequested: f.clearRequested,
      },
    ]),
  );
  store.initialize(models, map);
  return store;
}

function makeFormApi(fields: FormFieldState[]): XmlFormApi {
  const actions = {
    setFieldValue: vi.fn(),
    focusField: vi.fn(),
    blurField: vi.fn(),
    resetField: vi.fn(),
    clearField: vi.fn(),
    discardDrafts: vi.fn(),
    flushField: vi.fn().mockResolvedValue(null),
    flushAll: vi.fn().mockResolvedValue(null),
  };
  return {
    snapshot: { defNodeId: 1, fields },
    store: makeStore(fields),
    actions,
    hasDraftChanges: false,
    hasPendingCommits: false,
    hasBlockingErrors: false,
    formError: null,
    setFieldValue: actions.setFieldValue,
    focusField: actions.focusField,
    blurField: actions.blurField,
    resetField: actions.resetField,
    clearField: actions.clearField,
    discardDrafts: actions.discardDrafts,
    flushField: actions.flushField,
    flushAll: actions.flushAll,
  };
}

describe("XmlFormEditor", () => {
  describe("section header grouping", () => {
    it("renders Graphic Data header exactly once despite namedMap and nestedListItems interruptions", () => {
      // cornerOverlayPath comes before the damageData sub-section so the parent
      // graphicData section remains contiguous - matching real schema field order.
      const fields: FormFieldState[] = [
        makeField(
          "texPath",
          "graphicData.texPath",
          "Tex Path",
          {
            kind: "nestedObjectField",
            objectPath: ["graphicData"],
            fieldName: "texPath",
          },
          "text",
          0,
        ),
        makeField(
          "shaderParameters",
          "graphicData.shaderParameters",
          "Shader Parameters",
          {
            kind: "namedMap",
            objectPath: ["graphicData"],
            mapName: "shaderParameters",
          },
          "namedMap",
          1,
        ),
        makeField(
          "color",
          "graphicData.color",
          "Color",
          {
            kind: "nestedObjectField",
            objectPath: ["graphicData"],
            fieldName: "color",
          },
          "text",
          2,
        ),
        makeField(
          "linkFlags",
          "graphicData.linkFlags",
          "Link Flags",
          {
            kind: "nestedListItems",
            objectPath: ["graphicData"],
            fieldName: "linkFlags",
          },
          "list",
          3,
        ),
        makeField(
          "cornerOverlay",
          "graphicData.cornerOverlayPath",
          "Corner Overlay Path",
          {
            kind: "nestedObjectField",
            objectPath: ["graphicData"],
            fieldName: "cornerOverlayPath",
          },
          "text",
          4,
        ),
        makeField(
          "damageEnabled",
          "graphicData.damageData.enabled",
          "Enabled",
          {
            kind: "nestedObjectField",
            objectPath: ["graphicData", "damageData"],
            fieldName: "enabled",
          },
          "text",
          5,
        ),
      ];

      render(
        <XmlEditorContextProvider value={{ readOnly: false }}>
          <XmlFormEditor
            snapshot={makeSnapshot()}
            selectedDefNodeId={1}
            onSelectDef={vi.fn()}
            formApi={makeFormApi(fields)}
          />
        </XmlEditorContextProvider>,
      );

      expect(screen.getAllByText("Graphic Data")).toHaveLength(1);
      expect(screen.getAllByText("Graphic Data / Damage Data")).toHaveLength(1);
    });

    it("does not emit duplicate section headers when objectList fields interrupt nestedObjectField fields", () => {
      const fields: FormFieldState[] = [
        makeField(
          "texPath",
          "graphicData.texPath",
          "Tex Path",
          {
            kind: "nestedObjectField",
            objectPath: ["graphicData"],
            fieldName: "texPath",
          },
          "text",
          0,
        ),
        makeField(
          "attachments",
          "graphicData.attachments",
          "Attachments",
          {
            kind: "objectList",
            objectPath: ["graphicData"],
            fieldName: "attachments",
          },
          "objectList",
          1,
        ),
        makeField(
          "graphicClass",
          "graphicData.graphicClass",
          "Graphic Class",
          {
            kind: "nestedObjectField",
            objectPath: ["graphicData"],
            fieldName: "graphicClass",
          },
          "text",
          2,
        ),
      ];

      render(
        <XmlEditorContextProvider value={{ readOnly: false }}>
          <XmlFormEditor
            snapshot={makeSnapshot()}
            selectedDefNodeId={1}
            onSelectDef={vi.fn()}
            formApi={makeFormApi(fields)}
          />
        </XmlEditorContextProvider>,
      );

      expect(screen.getAllByText("Graphic Data")).toHaveLength(1);
    });

    it("renders top-level fields without a section header", () => {
      const fields: FormFieldState[] = [
        makeField(
          "defName",
          "defName",
          "Def Name",
          { kind: "childElement", childName: "defName" },
          "text",
          0,
        ),
        makeField(
          "description",
          "description",
          "Description",
          { kind: "childElement", childName: "description" },
          "text",
          1,
        ),
      ];

      render(
        <XmlEditorContextProvider value={{ readOnly: false }}>
          <XmlFormEditor
            snapshot={makeSnapshot()}
            selectedDefNodeId={1}
            onSelectDef={vi.fn()}
            formApi={makeFormApi(fields)}
          />
        </XmlEditorContextProvider>,
      );

      expect(screen.queryByText(/Graphic Data/)).toBeNull();
      expect(screen.getByLabelText("Def Name")).not.toBeNull();
      expect(screen.getByLabelText("Description")).not.toBeNull();
    });
  });

  describe("collapsible sections", () => {
    function makeNestedField(
      id: string,
      key: string,
      label: string,
      objectPath: string[],
      fieldName: string,
      sectionDefaultCollapsed?: boolean,
      sectionHasData?: boolean,
      order: number = 0,
    ): FormFieldState {
      const field = makeField(
        id,
        key,
        label,
        { kind: "nestedObjectField", objectPath, fieldName },
        "text",
        order,
      );
      const sectionEntry: FormSectionDefaults = {
        path: objectPath,
        defaultCollapsed: sectionDefaultCollapsed,
        hasData: sectionHasData,
      };
      return {
        ...field,
        model: { ...field.model, sectionDefaults: [sectionEntry] },
      };
    }

    it("explicit defaultCollapsed: true collapses section initially", () => {
      const fields: FormFieldState[] = [
        makeNestedField(
          "texPath",
          "graphicData.texPath",
          "Tex Path",
          ["graphicData"],
          "texPath",
          true,
          true,
        ),
      ];

      render(
        <XmlEditorContextProvider value={{ readOnly: false }}>
          <XmlFormEditor
            snapshot={makeSnapshot()}
            selectedDefNodeId={1}
            onSelectDef={vi.fn()}
            formApi={makeFormApi(fields)}
          />
        </XmlEditorContextProvider>,
      );

      expect(
        screen.getByRole("button", { name: "Graphic Data" }),
      ).not.toBeNull();
      expect(screen.queryByLabelText("Tex Path")).toBeNull();
    });

    it("explicit defaultCollapsed: false opens section initially", () => {
      const fields: FormFieldState[] = [
        makeNestedField(
          "texPath",
          "graphicData.texPath",
          "Tex Path",
          ["graphicData"],
          "texPath",
          false,
          false,
        ),
      ];

      render(
        <XmlEditorContextProvider value={{ readOnly: false }}>
          <XmlFormEditor
            snapshot={makeSnapshot()}
            selectedDefNodeId={1}
            onSelectDef={vi.fn()}
            formApi={makeFormApi(fields)}
          />
        </XmlEditorContextProvider>,
      );

      expect(screen.getByLabelText("Tex Path")).not.toBeNull();
    });

    it("missing defaultCollapsed with no data collapses section", () => {
      const fields: FormFieldState[] = [
        makeNestedField(
          "texPath",
          "graphicData.texPath",
          "Tex Path",
          ["graphicData"],
          "texPath",
          undefined,
          false,
        ),
      ];

      render(
        <XmlEditorContextProvider value={{ readOnly: false }}>
          <XmlFormEditor
            snapshot={makeSnapshot()}
            selectedDefNodeId={1}
            onSelectDef={vi.fn()}
            formApi={makeFormApi(fields)}
          />
        </XmlEditorContextProvider>,
      );

      expect(screen.queryByLabelText("Tex Path")).toBeNull();
    });

    it("missing defaultCollapsed with data opens section", () => {
      const fields: FormFieldState[] = [
        makeNestedField(
          "texPath",
          "graphicData.texPath",
          "Tex Path",
          ["graphicData"],
          "texPath",
          undefined,
          true,
        ),
      ];

      render(
        <XmlEditorContextProvider value={{ readOnly: false }}>
          <XmlFormEditor
            snapshot={makeSnapshot()}
            selectedDefNodeId={1}
            onSelectDef={vi.fn()}
            formApi={makeFormApi(fields)}
          />
        </XmlEditorContextProvider>,
      );

      expect(screen.getByLabelText("Tex Path")).not.toBeNull();
    });

    it("clicking section header toggles visibility", async () => {
      const { userEvent } = await import("@testing-library/user-event");
      const user = userEvent.setup();

      const fields: FormFieldState[] = [
        makeNestedField(
          "texPath",
          "graphicData.texPath",
          "Tex Path",
          ["graphicData"],
          "texPath",
          false,
          true,
        ),
      ];

      render(
        <XmlEditorContextProvider value={{ readOnly: false }}>
          <XmlFormEditor
            snapshot={makeSnapshot()}
            selectedDefNodeId={1}
            onSelectDef={vi.fn()}
            formApi={makeFormApi(fields)}
          />
        </XmlEditorContextProvider>,
      );

      // Section starts open
      expect(screen.getByLabelText("Tex Path")).not.toBeNull();

      // Click to collapse
      await user.click(screen.getByRole("button", { name: "Graphic Data" }));
      expect(screen.queryByLabelText("Tex Path")).toBeNull();

      // Click to re-expand
      await user.click(screen.getByRole("button", { name: "Graphic Data" }));
      expect(screen.getByLabelText("Tex Path")).not.toBeNull();
    });

    it("nested subsections collapse independently from their parent", () => {
      const fields: FormFieldState[] = [
        makeNestedField(
          "texPath",
          "graphicData.texPath",
          "Tex Path",
          ["graphicData"],
          "texPath",
          false,
          true,
          0,
        ),
        makeNestedField(
          "volume",
          "graphicData.shadowData.volume",
          "Volume",
          ["graphicData", "shadowData"],
          "volume",
          true,
          false,
          1,
        ),
      ];

      render(
        <XmlEditorContextProvider value={{ readOnly: false }}>
          <XmlFormEditor
            snapshot={makeSnapshot()}
            selectedDefNodeId={1}
            onSelectDef={vi.fn()}
            formApi={makeFormApi(fields)}
          />
        </XmlEditorContextProvider>,
      );

      // Parent section is open
      expect(screen.getByLabelText("Tex Path")).not.toBeNull();
      // Nested subsection is collapsed
      expect(screen.queryByLabelText("Volume")).toBeNull();
      // Both headers visible
      expect(
        screen.getByRole("button", { name: "Graphic Data" }),
      ).not.toBeNull();
      expect(
        screen.getByRole("button", { name: "Graphic Data / Shadow Data" }),
      ).not.toBeNull();
    });
  });
});

// --- Fixture-backed GraphicDataPreview integration tests ---

describe("XmlFormEditor – GraphicDataPreview integration", () => {
  it("passes fixture texPath and graphicClass to GraphicDataPreview when graphicData section renders", () => {
    capturedPreviewProps.length = 0;

    const texPathState = makeGraphicDataFormState();
    const graphicClassState = makeGraphicDataFormState({
      model: {
        ...makeGraphicDataFormState().model,
        id: "graphicData.graphicClass",
        key: "graphicData.graphicClass",
        label: "Graphic Class",
        path: {
          kind: "nestedObjectField",
          objectPath: ["graphicData"],
          fieldName: "graphicClass",
        },
        fieldPath: ["graphicData", "graphicClass"],
        order: 1,
      },
      value: { kind: "scalar", value: "Graphic_Single" },
      initialValue: { kind: "scalar", value: "Graphic_Single" },
    });

    const fields = [texPathState, graphicClassState];

    render(
      <XmlEditorContextProvider value={{ readOnly: false }}>
        <XmlFormEditor
          snapshot={makeSnapshot()}
          selectedDefNodeId={1}
          onSelectDef={vi.fn()}
          formApi={makeFormApi(fields)}
        />
      </XmlEditorContextProvider>,
    );

    expect(capturedPreviewProps.length).toBeGreaterThan(0);
    const lastProps = capturedPreviewProps[capturedPreviewProps.length - 1];
    expect(lastProps.texPath).toBe("Things/Fixture/Single/FixtureSingle");
    expect(lastProps.graphicClass).toBe("Graphic_Single");
  });

  it("passes updated draft texPath and graphicClass to GraphicDataPreview after rerender", () => {
    capturedPreviewProps.length = 0;

    const makeFields = (
      texPath: string,
      graphicClass: string,
    ): FormFieldState[] => [
      makeGraphicDataFormState({
        value: { kind: "scalar", value: texPath },
        initialValue: { kind: "scalar", value: texPath },
      }),
      makeGraphicDataFormState({
        model: {
          ...makeGraphicDataFormState().model,
          id: "graphicData.graphicClass",
          key: "graphicData.graphicClass",
          label: "Graphic Class",
          path: {
            kind: "nestedObjectField",
            objectPath: ["graphicData"],
            fieldName: "graphicClass",
          },
          fieldPath: ["graphicData", "graphicClass"],
          order: 1,
        },
        value: { kind: "scalar", value: graphicClass },
        initialValue: { kind: "scalar", value: graphicClass },
      }),
    ];

    const { rerender } = render(
      <XmlEditorContextProvider value={{ readOnly: false }}>
        <XmlFormEditor
          snapshot={makeSnapshot()}
          selectedDefNodeId={1}
          onSelectDef={vi.fn()}
          formApi={makeFormApi(
            makeFields("Things/Fixture/Single/FixtureSingle", "Graphic_Single"),
          )}
        />
      </XmlEditorContextProvider>,
    );

    capturedPreviewProps.length = 0;

    rerender(
      <XmlEditorContextProvider value={{ readOnly: false }}>
        <XmlFormEditor
          snapshot={makeSnapshot()}
          selectedDefNodeId={1}
          onSelectDef={vi.fn()}
          formApi={makeFormApi(
            makeFields("Things/Changed/NewPath", "Graphic_Multi"),
          )}
        />
      </XmlEditorContextProvider>,
    );

    expect(capturedPreviewProps.length).toBeGreaterThan(0);
    const lastProps = capturedPreviewProps[capturedPreviewProps.length - 1];
    expect(lastProps.texPath).toBe("Things/Changed/NewPath");
    expect(lastProps.graphicClass).toBe("Graphic_Multi");
  });
});

// --- Issue 05: XmlFormEditor is agnostic to Form View filtering, but a hidden root's
// models simply never reach it (that's the whole point of filtering upstream in
// `buildFormDescriptors`/`useXmlFormController`, per Plan.md section 10). These tests
// simulate that upstream outcome directly - a model list with the graphicData root
// omitted - and prove XmlFormEditor never mounts its section header or the expensive
// GraphicDataPreviewConnected/GraphicDataPreview subtree for it.
describe("XmlFormEditor – hidden Form View roots never mount (issue 05)", () => {
  it("does not render the Graphic Data section or mount GraphicDataPreview when its models are absent", () => {
    capturedPreviewProps.length = 0;

    // Only a top-level scalar field is present - as if graphicData were the hidden root
    // of a Form View and `buildFormDescriptors` skipped its expansion entirely.
    const fields: FormFieldState[] = [
      makeField("defName", "defName", "Def Name", { kind: "childElement", childName: "defName" }, "text", 0),
    ];

    render(
      <XmlEditorContextProvider value={{ readOnly: false }}>
        <XmlFormEditor
          snapshot={makeSnapshot()}
          selectedDefNodeId={1}
          onSelectDef={vi.fn()}
          formApi={makeFormApi(fields)}
        />
      </XmlEditorContextProvider>,
    );

    expect(screen.queryByText("Graphic Data")).toBeNull();
    // GraphicDataPreviewConnected (and, transitively, GraphicDataPreview) never mounts,
    // so it never subscribes to the store or renders - proving the hidden root's
    // expensive preview subtree incurs no cost, not merely that it's visually absent.
    expect(capturedPreviewProps.length).toBe(0);
  });

  it("mounts only the visible sibling section when one of two object roots is hidden", () => {
    capturedPreviewProps.length = 0;

    // graphicData is present (visible); a hypothetical second object-root section
    // ("comps") is entirely absent, as it would be if hidden by a Form View.
    const fields: FormFieldState[] = [
      makeGraphicDataFormState(),
      makeGraphicDataFormState({
        model: {
          ...makeGraphicDataFormState().model,
          id: "graphicData.graphicClass",
          key: "graphicData.graphicClass",
          label: "Graphic Class",
          path: {
            kind: "nestedObjectField",
            objectPath: ["graphicData"],
            fieldName: "graphicClass",
          },
          fieldPath: ["graphicData", "graphicClass"],
          order: 1,
        },
        value: { kind: "scalar", value: "Graphic_Single" },
        initialValue: { kind: "scalar", value: "Graphic_Single" },
      }),
    ];

    render(
      <XmlEditorContextProvider value={{ readOnly: false }}>
        <XmlFormEditor
          snapshot={makeSnapshot()}
          selectedDefNodeId={1}
          onSelectDef={vi.fn()}
          formApi={makeFormApi(fields)}
        />
      </XmlEditorContextProvider>,
    );

    expect(screen.getAllByText("Graphic Data")).toHaveLength(1);
    expect(screen.queryByText("Comps")).toBeNull();
    expect(capturedPreviewProps.length).toBeGreaterThan(0);
  });
});

function makeFormViewsController(
  overrides: Partial<UseFormViewsResult> = {},
): UseFormViewsResult {
  const defaultView: ResolvedFormView = {
    id: "default",
    targetDefType: "ThingDef",
    label: "Default View",
    order: 0,
    origin: "default",
    hiddenFieldIds: [],
    recommended: false,
  };
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
    saveOverrideAsCustomView: vi.fn(),
    duplicateAsCustomView: vi.fn(),
    createCustomView: vi.fn(),
    renameCustomView: vi.fn(),
    updateCustomView: vi.fn(),
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

describe("XmlFormEditor – Form View selector placement (issue 06)", () => {
  it("renders the selector below the multi-Def selector and above .fields when applicable", () => {
    const snapshot = makeSnapshot();
    // A second Def so the multi-Def selector itself also renders, to prove ordering.
    snapshot.parsed!.defs.push({ ...snapshot.parsed!.defs[0], nodeId: 2, defName: "Second" });

    render(
      <XmlEditorContextProvider value={{ readOnly: false }}>
        <XmlFormEditor
          snapshot={snapshot}
          selectedDefNodeId={1}
          onSelectDef={vi.fn()}
          formApi={makeFormApi([])}
          formViews={makeFormViewsController()}
        />
      </XmlEditorContextProvider>,
    );

    const defSelectorLabel = screen.getByText("Def");
    const viewLabel = screen.getByText("View");
    const noFieldsText = screen.getByText(/No schema available/);

    // DOM order proves placement: Def selector, then Form View selector, then `.fields`.
    // eslint-disable-next-line no-bitwise
    expect(
      defSelectorLabel.compareDocumentPosition(viewLabel) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    // eslint-disable-next-line no-bitwise
    expect(
      viewLabel.compareDocumentPosition(noFieldsText) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });

  it("renders no Form View controls when the controller reports not applicable", () => {
    render(
      <XmlEditorContextProvider value={{ readOnly: false }}>
        <XmlFormEditor
          snapshot={makeSnapshot()}
          selectedDefNodeId={1}
          onSelectDef={vi.fn()}
          formApi={makeFormApi([])}
          formViews={makeFormViewsController({ applicable: false })}
        />
      </XmlEditorContextProvider>,
    );

    expect(screen.queryByText("View")).toBeNull();
    expect(screen.queryByText("Customize view")).toBeNull();
  });

  it("renders no Form View controls when no controller is supplied at all (Patch/About/raw parity)", () => {
    render(
      <XmlEditorContextProvider value={{ readOnly: false }}>
        <XmlFormEditor
          snapshot={makeSnapshot()}
          selectedDefNodeId={1}
          onSelectDef={vi.fn()}
          formApi={makeFormApi([])}
        />
      </XmlEditorContextProvider>,
    );

    expect(screen.queryByText("View")).toBeNull();
  });

  it("opens the Form View manager dialog from Customize view and closes it again", async () => {
    const { default: userEvent } = await import("@testing-library/user-event");
    render(
      <XmlEditorContextProvider value={{ readOnly: false }}>
        <XmlFormEditor
          snapshot={makeSnapshot()}
          selectedDefNodeId={1}
          onSelectDef={vi.fn()}
          formApi={makeFormApi([])}
          formViews={makeFormViewsController()}
        />
      </XmlEditorContextProvider>,
    );

    expect(screen.queryByText("Manage Form Views")).toBeNull();
    await userEvent.click(screen.getByText("Customize view"));
    expect(screen.getByText("Manage Form Views")).toBeTruthy();
    await userEvent.click(screen.getByLabelText("Close"));
    expect(screen.queryByText("Manage Form Views")).toBeNull();
  });
});

describe("XmlFormEditor - customize mode inline hide/show controls (issue 07)", () => {
  function fields(): FormFieldState[] {
    const topLevel = makeField(
      "defName",
      "defName",
      "Def Name",
      { kind: "childElement", childName: "defName" },
      "text",
      0,
    );
    const nestedTex = makeField(
      "graphicData.texPath",
      "graphicData.texPath",
      "Tex Path",
      { kind: "nestedObjectField", objectPath: ["graphicData"], fieldName: "texPath" },
      "text",
      1,
    );
    const withSection: FormFieldState = {
      ...nestedTex,
      model: {
        ...nestedTex.model,
        sectionDefaults: [{ path: ["graphicData"], defaultCollapsed: false, hasData: true }],
      },
    };
    return [topLevel, withSection];
  }

  it("renders no inline hide button on a top-level field or section header outside customize mode", () => {
    render(
      <XmlEditorContextProvider value={{ readOnly: false }}>
        <XmlFormEditor
          snapshot={makeSnapshot()}
          selectedDefNodeId={1}
          onSelectDef={vi.fn()}
          formApi={makeFormApi(fields())}
          formViews={makeFormViewsController()}
        />
      </XmlEditorContextProvider>,
    );

    expect(screen.queryByLabelText("Hide Def Name")).toBeNull();
    expect(screen.queryByLabelText("Hide Graphic Data")).toBeNull();
  });

  it("shows inline hide buttons only once customize mode (the manager dialog) is opened, toggling the same override state as the checklist", async () => {
    const { default: userEvent } = await import("@testing-library/user-event");
    const formViews = makeFormViewsController();
    render(
      <XmlEditorContextProvider value={{ readOnly: false }}>
        <XmlFormEditor
          snapshot={makeSnapshot()}
          selectedDefNodeId={1}
          onSelectDef={vi.fn()}
          formApi={makeFormApi(fields())}
          formViews={formViews}
        />
      </XmlEditorContextProvider>,
    );

    expect(screen.queryByLabelText("Hide Def Name")).toBeNull();
    await userEvent.click(screen.getByText("Customize view"));

    const hideField = screen.getByLabelText("Hide Def Name");
    const hideSection = screen.getByLabelText("Hide Graphic Data");
    expect(hideField).toBeTruthy();
    expect(hideSection).toBeTruthy();

    await userEvent.click(hideField);
    // Same shared toggle primitive/controller method the checklist's checkboxes call --
    // `effectiveHidden` (empty here) with "defName" added.
    expect(formViews.setOverrideHiddenFieldIds).toHaveBeenCalledWith(new Set(["defName"]));

    await userEvent.click(hideSection);
    expect(formViews.setOverrideHiddenFieldIds).toHaveBeenCalledWith(new Set(["graphicData"]));
  });

  it("does not render inline hide buttons when Form Views are not applicable", () => {
    render(
      <XmlEditorContextProvider value={{ readOnly: false }}>
        <XmlFormEditor
          snapshot={makeSnapshot()}
          selectedDefNodeId={1}
          onSelectDef={vi.fn()}
          formApi={makeFormApi(fields())}
          formViews={makeFormViewsController({ applicable: false })}
        />
      </XmlEditorContextProvider>,
    );

    // No "Customize view" entry point at all when not applicable (issue 06 contract), so
    // customize mode can never be entered and no inline controls ever render.
    expect(screen.queryByText("Customize view")).toBeNull();
    expect(screen.queryByLabelText("Hide Def Name")).toBeNull();
  });
});

// --- Issue 08: hidden validation feedback (header summary, reveal, focus-after-reveal) -----

describe("XmlFormEditor - hidden field validation feedback (issue 08)", () => {
  // A Def-root-level diagnostic (`nodeId` equal to the Def's own node id, exactly like Rust's
  // `validate_required_fields_present`/`validate_def_identity` emit for a missing required field
  // or duplicate defName - see `xml_document/validation/document.rs`), mapping to the
  // `graphicData` root via `fieldPath`.
  function hiddenGraphicDataDiagnostic(blocking: boolean) {
    return {
      relativePath: "Things/Test.xml",
      nodeId: 1,
      line: null,
      column: null,
      severity: blocking ? ("Error" as const) : ("Warning" as const),
      message: "test diagnostic",
      code: blocking ? "validation_field_type_mismatch" : "validation_field_shape_mismatch",
      defType: "ThingDef",
      defName: "Test",
      fieldPath: "graphicData",
      blocking,
    };
  }

  // Only a top-level scalar field is present - as if `graphicData` were hidden by the active
  // Form View and `buildFormDescriptors` skipped its expansion (issue 05's contract).
  function fieldsWithGraphicDataHidden(): FormFieldState[] {
    return [
      makeField("defName", "defName", "Def Name", { kind: "childElement", childName: "defName" }, "text", 0),
    ];
  }

  // Simulates the post-reveal rebuild: the same top-level field, plus `graphicData.texPath` now
  // present because the override no longer hides its root.
  function fieldsWithGraphicDataRevealed(): FormFieldState[] {
    return [
      ...fieldsWithGraphicDataHidden(),
      makeField(
        "graphicData.texPath",
        "graphicData.texPath",
        "Tex Path",
        { kind: "nestedObjectField", objectPath: ["graphicData"], fieldName: "texPath" },
        "text",
        1,
      ),
    ];
  }

  function snapshotWithHiddenDiagnostic(blocking: boolean): XmlEditorSnapshot {
    const snapshot = makeSnapshot();
    snapshot.validationDiagnostics = [hiddenGraphicDataDiagnostic(blocking)];
    return snapshot;
  }

  it("renders no summary when there are no diagnostics mapping to a hidden root", () => {
    render(
      <XmlEditorContextProvider value={{ readOnly: false }}>
        <XmlFormEditor
          snapshot={makeSnapshot()}
          selectedDefNodeId={1}
          onSelectDef={vi.fn()}
          formApi={makeFormApi(fieldsWithGraphicDataHidden())}
          formViews={makeFormViewsController({ effectiveHidden: new Set(["graphicData"]) })}
        />
      </XmlEditorContextProvider>,
    );

    expect(screen.queryByText(/hidden field issue/)).toBeNull();
  });

  it("renders the hidden issue count and blocking qualifier, without changing any state on its own (no auto-reveal)", () => {
    const formViews = makeFormViewsController({ effectiveHidden: new Set(["graphicData"]) });
    render(
      <XmlEditorContextProvider value={{ readOnly: false }}>
        <XmlFormEditor
          snapshot={snapshotWithHiddenDiagnostic(true)}
          selectedDefNodeId={1}
          onSelectDef={vi.fn()}
          formApi={makeFormApi(fieldsWithGraphicDataHidden())}
          formViews={formViews}
        />
      </XmlEditorContextProvider>,
    );

    expect(screen.getByText("1 hidden field issue (1 blocking)")).toBeTruthy();
    // Merely rendering with a hidden diagnostic present must never itself call the override
    // setter - only the explicit `Reveal fields with issues` click may.
    expect(formViews.setOverrideHiddenFieldIds).not.toHaveBeenCalled();
  });

  it("does not count a diagnostic whose root is not currently hidden", () => {
    render(
      <XmlEditorContextProvider value={{ readOnly: false }}>
        <XmlFormEditor
          snapshot={snapshotWithHiddenDiagnostic(true)}
          selectedDefNodeId={1}
          onSelectDef={vi.fn()}
          formApi={makeFormApi(fieldsWithGraphicDataRevealed())}
          formViews={makeFormViewsController({ effectiveHidden: new Set() })}
        />
      </XmlEditorContextProvider>,
    );

    expect(screen.queryByText(/hidden field issue/)).toBeNull();
  });

  it("clicking Reveal fields with issues unhides only the affected roots via setOverrideHiddenFieldIds, leaving other hidden roots intact", async () => {
    const { default: userEvent } = await import("@testing-library/user-event");
    const formViews = makeFormViewsController({
      effectiveHidden: new Set(["graphicData", "comps"]),
    });
    render(
      <XmlEditorContextProvider value={{ readOnly: false }}>
        <XmlFormEditor
          snapshot={snapshotWithHiddenDiagnostic(true)}
          selectedDefNodeId={1}
          onSelectDef={vi.fn()}
          formApi={makeFormApi(fieldsWithGraphicDataHidden())}
          formViews={formViews}
        />
      </XmlEditorContextProvider>,
    );

    await userEvent.click(screen.getByText("Reveal fields with issues"));

    // "comps" stays hidden - only "graphicData" (the affected root) is unhidden. Never a Custom
    // View mutation, never `selectView` (no view switch, no confirmation prompt).
    expect(formViews.setOverrideHiddenFieldIds).toHaveBeenCalledTimes(1);
    expect(formViews.setOverrideHiddenFieldIds).toHaveBeenCalledWith(new Set(["comps"]));
    expect(formViews.selectView).not.toHaveBeenCalled();
  });

  it("focuses the first newly-revealed field once the form rebuilds after a reveal", async () => {
    const { default: userEvent } = await import("@testing-library/user-event");
    const formViews = makeFormViewsController({ effectiveHidden: new Set(["graphicData"]) });
    const { rerender } = render(
      <XmlEditorContextProvider value={{ readOnly: false }}>
        <XmlFormEditor
          snapshot={snapshotWithHiddenDiagnostic(true)}
          selectedDefNodeId={1}
          onSelectDef={vi.fn()}
          formApi={makeFormApi(fieldsWithGraphicDataHidden())}
          formViews={formViews}
        />
      </XmlEditorContextProvider>,
    );

    await userEvent.click(screen.getByText("Reveal fields with issues"));
    expect(formViews.setOverrideHiddenFieldIds).toHaveBeenCalledWith(new Set());

    // Simulate the real app: the override change flows back through `useFormViews` ->
    // `visibleTopLevelFieldIds` -> `useXmlFormController`'s rebuild, which here is stood in for
    // by re-rendering with a fresh `formApi`/store whose models now include the revealed field.
    rerender(
      <XmlEditorContextProvider value={{ readOnly: false }}>
        <XmlFormEditor
          snapshot={snapshotWithHiddenDiagnostic(true)}
          selectedDefNodeId={1}
          onSelectDef={vi.fn()}
          formApi={makeFormApi(fieldsWithGraphicDataRevealed())}
          formViews={makeFormViewsController({ effectiveHidden: new Set() })}
        />
      </XmlEditorContextProvider>,
    );

    const revealedInput = screen.getByLabelText("Tex Path");
    expect(document.activeElement).toBe(revealedInput);
  });

  // Simulates the post-reveal rebuild for an object-root field the user never populated: its
  // section metadata reports `hasData: false` with no explicit `defaultCollapsed`, which
  // `computeInitialCollapsed` collapses by default (this is exactly why the field had a
  // "missing required field" diagnostic in the first place - a very plausible/common reveal
  // target, not an edge case).
  function fieldsWithGraphicDataRevealedNoData(): FormFieldState[] {
    const texPath = makeField(
      "graphicData.texPath",
      "graphicData.texPath",
      "Tex Path",
      { kind: "nestedObjectField", objectPath: ["graphicData"], fieldName: "texPath" },
      "text",
      1,
    );
    const withCollapsedByDefaultSection: FormFieldState = {
      ...texPath,
      model: {
        ...texPath.model,
        sectionDefaults: [{ path: ["graphicData"], defaultCollapsed: undefined, hasData: false }],
      },
    };
    return [...fieldsWithGraphicDataHidden(), withCollapsedByDefaultSection];
  }

  it("expands a section that defaults to collapsed (no existing data) before focusing its revealed field", async () => {
    const { default: userEvent } = await import("@testing-library/user-event");
    const formViews = makeFormViewsController({ effectiveHidden: new Set(["graphicData"]) });
    const { rerender } = render(
      <XmlEditorContextProvider value={{ readOnly: false }}>
        <XmlFormEditor
          snapshot={snapshotWithHiddenDiagnostic(true)}
          selectedDefNodeId={1}
          onSelectDef={vi.fn()}
          formApi={makeFormApi(fieldsWithGraphicDataHidden())}
          formViews={formViews}
        />
      </XmlEditorContextProvider>,
    );

    await userEvent.click(screen.getByText("Reveal fields with issues"));
    expect(formViews.setOverrideHiddenFieldIds).toHaveBeenCalledWith(new Set());

    rerender(
      <XmlEditorContextProvider value={{ readOnly: false }}>
        <XmlFormEditor
          snapshot={snapshotWithHiddenDiagnostic(true)}
          selectedDefNodeId={1}
          onSelectDef={vi.fn()}
          formApi={makeFormApi(fieldsWithGraphicDataRevealedNoData())}
          formViews={makeFormViewsController({ effectiveHidden: new Set() })}
        />
      </XmlEditorContextProvider>,
    );

    // Without the reveal flow forcing the section open, this section would default to
    // collapsed (`hasData: false`, no explicit `defaultCollapsed`) and "Tex Path" would never
    // mount at all - proving this is not the pre-existing "sections start open" test setup.
    const header = screen.getByRole("button", { name: "Graphic Data" });
    expect(header.getAttribute("aria-expanded")).toBe("true");

    const revealedInput = screen.getByLabelText("Tex Path");
    expect(document.activeElement).toBe(revealedInput);
  });

  // Simulates a field 3+ levels deep: `graphicData` (root section) > `shadowData` (subsection)
  // > `volume` (the field itself), with BOTH ancestor sections reporting `hasData: false` (so
  // both would default to collapsed on their own, not just the outer one).
  function fieldsWithDeepNestingRevealed(): FormFieldState[] {
    const volume = makeField(
      "graphicData.shadowData.volume",
      "graphicData.shadowData.volume",
      "Volume",
      { kind: "nestedObjectField", objectPath: ["graphicData", "shadowData"], fieldName: "volume" },
      "text",
      1,
    );
    const withSections: FormFieldState = {
      ...volume,
      model: {
        ...volume.model,
        sectionDefaults: [
          { path: ["graphicData"], defaultCollapsed: undefined, hasData: false },
          { path: ["graphicData", "shadowData"], defaultCollapsed: undefined, hasData: false },
        ],
      },
    };
    return [...fieldsWithGraphicDataHidden(), withSections];
  }

  it("expands every ancestor section (3+ levels deep) before focusing a deeply-nested revealed field", async () => {
    const { default: userEvent } = await import("@testing-library/user-event");
    const formViews = makeFormViewsController({ effectiveHidden: new Set(["graphicData"]) });
    const { rerender } = render(
      <XmlEditorContextProvider value={{ readOnly: false }}>
        <XmlFormEditor
          snapshot={snapshotWithHiddenDiagnostic(true)}
          selectedDefNodeId={1}
          onSelectDef={vi.fn()}
          formApi={makeFormApi(fieldsWithGraphicDataHidden())}
          formViews={formViews}
        />
      </XmlEditorContextProvider>,
    );

    await userEvent.click(screen.getByText("Reveal fields with issues"));
    expect(formViews.setOverrideHiddenFieldIds).toHaveBeenCalledWith(new Set());

    rerender(
      <XmlEditorContextProvider value={{ readOnly: false }}>
        <XmlFormEditor
          snapshot={snapshotWithHiddenDiagnostic(true)}
          selectedDefNodeId={1}
          onSelectDef={vi.fn()}
          formApi={makeFormApi(fieldsWithDeepNestingRevealed())}
          formViews={makeFormViewsController({ effectiveHidden: new Set() })}
        />
      </XmlEditorContextProvider>,
    );

    // Both the root section AND the subsection would independently default to collapsed - both
    // must be forced open, not just the outermost one, or "Volume" would still never mount.
    expect(screen.getByRole("button", { name: "Graphic Data" }).getAttribute("aria-expanded")).toBe(
      "true",
    );
    expect(
      screen.getByRole("button", { name: "Graphic Data / Shadow Data" }).getAttribute("aria-expanded"),
    ).toBe("true");

    const revealedInput = screen.getByLabelText("Volume");
    expect(document.activeElement).toBe(revealedInput);
  });

  it("handles rapid successive reveal clicks without crashing, recomputing the override fresh each time", async () => {
    const { default: userEvent } = await import("@testing-library/user-event");
    const formViews = makeFormViewsController({
      effectiveHidden: new Set(["graphicData", "comps"]),
    });
    render(
      <XmlEditorContextProvider value={{ readOnly: false }}>
        <XmlFormEditor
          snapshot={snapshotWithHiddenDiagnostic(true)}
          selectedDefNodeId={1}
          onSelectDef={vi.fn()}
          formApi={makeFormApi(fieldsWithGraphicDataHidden())}
          formViews={formViews}
        />
      </XmlEditorContextProvider>,
    );

    const revealButton = screen.getByText("Reveal fields with issues");
    // Two clicks before any rebuild ever lands (the mock controller never actually mutates
    // `effectiveHidden`, standing in for "the override commit is still in flight") - must not
    // throw, and each call independently computes the correct result rather than compounding.
    await userEvent.click(revealButton);
    await userEvent.click(revealButton);

    expect(formViews.setOverrideHiddenFieldIds).toHaveBeenCalledTimes(2);
    expect(formViews.setOverrideHiddenFieldIds).toHaveBeenNthCalledWith(1, new Set(["comps"]));
    expect(formViews.setOverrideHiddenFieldIds).toHaveBeenNthCalledWith(2, new Set(["comps"]));
  });

  // A structurally EMPTY object field (zero schema-declared child fields, no discriminator - the
  // real `ThingDef.colorGenerator`/`colorGeneratorInTraderStock` shape, whose `ColorGenerator`
  // object-type schema has `"fields": {}`) produces NO model at all after the rebuild, not even
  // a section header, independent of Form Views. There is nothing to focus for that specific
  // field; the fallback lands on the Form View selector control instead of silently doing
  // nothing.
  it("falls back to focusing the Form View selector when the revealed root produces no model at all (structurally empty object field)", async () => {
    const { default: userEvent } = await import("@testing-library/user-event");
    const emptyObjectDiagnostic = {
      relativePath: "Things/Test.xml",
      nodeId: 1,
      line: null,
      column: null,
      severity: "Warning" as const,
      message: "unknown field in colorGenerator",
      code: "validation_unknown_object_field",
      defType: "ThingDef",
      defName: "Test",
      fieldPath: "colorGenerator",
      blocking: false,
    };
    const snapshot = makeSnapshot();
    snapshot.validationDiagnostics = [emptyObjectDiagnostic];

    const formViews = makeFormViewsController({ effectiveHidden: new Set(["colorGenerator"]) });
    const { rerender } = render(
      <XmlEditorContextProvider value={{ readOnly: false }}>
        <XmlFormEditor
          snapshot={snapshot}
          selectedDefNodeId={1}
          onSelectDef={vi.fn()}
          formApi={makeFormApi(fieldsWithGraphicDataHidden())}
          formViews={formViews}
        />
      </XmlEditorContextProvider>,
    );

    await userEvent.click(screen.getByText("Reveal fields with issues"));
    expect(formViews.setOverrideHiddenFieldIds).toHaveBeenCalledWith(new Set());

    // Rebuild produces no "colorGenerator"-rooted model whatsoever - `defName` is still the
    // only model, exactly like `fieldsWithGraphicDataHidden()`, simulating that the newly
    // unhidden root rendered nothing.
    rerender(
      <XmlEditorContextProvider value={{ readOnly: false }}>
        <XmlFormEditor
          snapshot={snapshot}
          selectedDefNodeId={1}
          onSelectDef={vi.fn()}
          formApi={makeFormApi(fieldsWithGraphicDataHidden())}
          formViews={makeFormViewsController({ effectiveHidden: new Set() })}
        />
      </XmlEditorContextProvider>,
    );

    const selector = screen.getByLabelText("View") as HTMLSelectElement;
    expect(document.activeElement).toBe(selector);
  });
});

// --- Issue 08: reveal-focus never crosses into a different, mounted-but-inactive pane --------

describe("XmlFormEditor - reveal-focus is scoped per pane (issue 08 round 2)", () => {
  function hiddenGraphicDataDiagnostic(blocking: boolean) {
    return {
      relativePath: "Things/Test.xml",
      nodeId: 1,
      line: null,
      column: null,
      severity: blocking ? ("Error" as const) : ("Warning" as const),
      message: "test diagnostic",
      code: blocking ? "validation_field_type_mismatch" : "validation_field_shape_mismatch",
      defType: "ThingDef",
      defName: "Test",
      fieldPath: "graphicData",
      blocking,
    };
  }

  function fieldsGraphicDataHidden(): FormFieldState[] {
    return [
      makeField("defName", "defName", "Def Name", { kind: "childElement", childName: "defName" }, "text", 0),
    ];
  }

  function fieldsGraphicDataRevealed(): FormFieldState[] {
    return [
      ...fieldsGraphicDataHidden(),
      makeField(
        "graphicData.texPath",
        "graphicData.texPath",
        "Tex Path",
        { kind: "nestedObjectField", objectPath: ["graphicData"], fieldName: "texPath" },
        "text",
        1,
      ),
    ];
  }

  function makeFormViewsController(overrides: Partial<UseFormViewsResult> = {}): UseFormViewsResult {
    const defaultView: ResolvedFormView = {
      id: "default",
      targetDefType: "ThingDef",
      label: "Default View",
      order: 0,
      origin: "default",
      hiddenFieldIds: [],
      recommended: false,
    };
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
      saveOverrideAsCustomView: vi.fn(),
      duplicateAsCustomView: vi.fn(),
      createCustomView: vi.fn(),
      renameCustomView: vi.fn(),
      updateCustomView: vi.fn(),
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

  function snapshotWithHiddenDiagnostic(blocking: boolean): XmlEditorSnapshot {
    const snapshot = makeSnapshot();
    snapshot.validationDiagnostics = [hiddenGraphicDataDiagnostic(blocking)];
    return snapshot;
  }

  it("focuses the revealed field inside its OWN pane, never a different mounted-but-inactive tab with a colliding field DOM id", async () => {
    const { default: userEvent } = await import("@testing-library/user-event");
    const formViewsA = makeFormViewsController({ effectiveHidden: new Set(["graphicData"]) });
    const formViewsB = makeFormViewsController({ effectiveHidden: new Set() });

    // Pane B renders FIRST in document order and already has a "Tex Path" input mounted using
    // the exact SAME model id ("graphicData.texPath") that pane A's own reveal will produce -
    // i.e. the real DOM id collision `EditorWorkspace` keeping every open tab's pane mounted
    // (Plan.md section 9) makes possible when two tabs share a Def identity/node id. If
    // reveal-focus used any GLOBAL `document.getElementById`/`querySelector` lookup, it would
    // find this (wrong, inactive-tab) element first, since it comes first in document order.
    const tree = (paneAFields: FormFieldState[], paneAViews: UseFormViewsResult) => (
      <>
        <div data-testid="pane-b">
          <XmlEditorContextProvider value={{ readOnly: false }}>
            <XmlFormEditor
              snapshot={makeSnapshot()}
              selectedDefNodeId={1}
              onSelectDef={vi.fn()}
              formApi={makeFormApi(fieldsGraphicDataRevealed())}
              formViews={formViewsB}
            />
          </XmlEditorContextProvider>
        </div>
        <div data-testid="pane-a">
          <XmlEditorContextProvider value={{ readOnly: false }}>
            <XmlFormEditor
              snapshot={snapshotWithHiddenDiagnostic(true)}
              selectedDefNodeId={1}
              onSelectDef={vi.fn()}
              formApi={makeFormApi(paneAFields)}
              formViews={paneAViews}
            />
          </XmlEditorContextProvider>
        </div>
      </>
    );

    const { rerender } = render(tree(fieldsGraphicDataHidden(), formViewsA));

    const paneA = screen.getByTestId("pane-a");
    await userEvent.click(within(paneA).getByText("Reveal fields with issues"));
    expect(formViewsA.setOverrideHiddenFieldIds).toHaveBeenCalledWith(new Set());

    // Simulate the rebuild landing only in pane A.
    rerender(tree(fieldsGraphicDataRevealed(), makeFormViewsController({ effectiveHidden: new Set() })));

    // Raw scoped `querySelector` rather than `getByLabelText` here, AND an `[id="..."]`
    // attribute selector rather than a `#id` selector: with a genuinely duplicate `id` in the
    // document, (a) the browser/jsdom `<label for>` -> control association itself becomes
    // ambiguous, and (b) jsdom's selector engine (`nwsapi`) takes a `getElementById`-style fast
    // path for a bare `#id` selector even when scoped to a subtree - silently returning null (or
    // the WRONG pane's element) whenever that id also exists elsewhere in the document, which is
    // exactly the situation under test here. An attribute selector has no such fast path and
    // always walks the real subtree - it is the same reason the production fix
    // (`findScopedById` in `XmlFormEditor.tsx`) uses one too.
    const paneAInput = screen
      .getByTestId("pane-a")
      .querySelector<HTMLInputElement>('[id="field-graphicData-texPath"]');
    const paneBInput = screen
      .getByTestId("pane-b")
      .querySelector<HTMLInputElement>('[id="field-graphicData-texPath"]');
    expect(paneAInput).not.toBeNull();
    expect(paneBInput).not.toBeNull();

    // Both panes render an element with the SAME (colliding) DOM id - proving this is the
    // scoping fix at work, not merely a lucky document-order coincidence.
    expect(paneAInput!.id).toBe(paneBInput!.id);
    expect(document.activeElement).toBe(paneAInput);
    expect(document.activeElement).not.toBe(paneBInput);
  });
});

// --- Issue 08 round 3: reveal-focus reaches objectList-control top-level fields ------------
//
// `ObjectListEditor` (list-of-objects fields like `verbs`/`comps` - one of the most common
// top-level field shapes in real RimWorld Def schemas) renders zero, one, or many nested item
// rows rather than a single `<input>`, so it has no natural place to put `field-${id}` the way
// every scalar/text control does. `ObjectListEditor`'s own root container now always carries
// that id (with `tabIndex={-1}` so it is programmatically focusable) regardless of how many
// items it currently has - see the `id`/`tabIndex` wiring in `ObjectListEditor.tsx` and
// `FormFieldControl.tsx`'s two `case "objectList"` render sites.

describe("XmlFormEditor - reveal-focus reaches objectList controls (issue 08 round 3)", () => {
  function hiddenVerbsDiagnostic(blocking: boolean) {
    return {
      relativePath: "Things/Test.xml",
      nodeId: 1,
      line: null,
      column: null,
      severity: blocking ? ("Error" as const) : ("Warning" as const),
      message: "test diagnostic",
      code: blocking ? "validation_field_type_mismatch" : "validation_field_shape_mismatch",
      defType: "ThingDef",
      defName: "Test",
      fieldPath: "verbs",
      blocking,
    };
  }

  function fieldsWithVerbsHidden(): FormFieldState[] {
    return [
      makeField("defName", "defName", "Def Name", { kind: "childElement", childName: "defName" }, "text", 0),
    ];
  }

  // A top-level `objectList`-control field (the real shape of e.g. `ThingDef.verbs`/`comps`)
  // with ZERO current items - the same "required field revealed, no data yet" scenario as the
  // collapsed-section tests above, and specifically the case where `ObjectListEditor` renders no
  // nested item row at all to carry a DOM id.
  function fieldsWithVerbsRevealed(): FormFieldState[] {
    const verbsField = makeField(
      "verbs",
      "verbs",
      "Verbs",
      { kind: "childElement", childName: "verbs" },
      "objectList",
      1,
    );
    const withObjectListValue: FormFieldState = {
      ...verbsField,
      model: { ...verbsField.model, itemSchemaRef: "VerbProperties" },
      value: { kind: "objectList", items: [] },
      initialValue: { kind: "objectList", items: [] },
    };
    return [...fieldsWithVerbsHidden(), withObjectListValue];
  }

  function snapshotWithHiddenVerbsDiagnostic(blocking: boolean): XmlEditorSnapshot {
    const snapshot = makeSnapshot();
    snapshot.validationDiagnostics = [hiddenVerbsDiagnostic(blocking)];
    return snapshot;
  }

  function makeFormViewsController(overrides: Partial<UseFormViewsResult> = {}): UseFormViewsResult {
    const defaultView: ResolvedFormView = {
      id: "default",
      targetDefType: "ThingDef",
      label: "Default View",
      order: 0,
      origin: "default",
      hiddenFieldIds: [],
      recommended: false,
    };
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
      saveOverrideAsCustomView: vi.fn(),
      duplicateAsCustomView: vi.fn(),
      createCustomView: vi.fn(),
      renameCustomView: vi.fn(),
      updateCustomView: vi.fn(),
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

  it("scrolls to and focuses a revealed top-level objectList control (e.g. verbs), not left stuck on the Reveal button", async () => {
    const { default: userEvent } = await import("@testing-library/user-event");
    const formViews = makeFormViewsController({ effectiveHidden: new Set(["verbs"]) });
    const { rerender } = render(
      <XmlEditorContextProvider value={{ readOnly: false }}>
        <XmlFormEditor
          snapshot={snapshotWithHiddenVerbsDiagnostic(true)}
          selectedDefNodeId={1}
          onSelectDef={vi.fn()}
          formApi={makeFormApi(fieldsWithVerbsHidden())}
          formViews={formViews}
        />
      </XmlEditorContextProvider>,
    );

    await userEvent.click(screen.getByText("Reveal fields with issues"));
    expect(formViews.setOverrideHiddenFieldIds).toHaveBeenCalledWith(new Set());

    // Simulate the rebuild landing with the objectList control's zero-item root now present.
    rerender(
      <XmlEditorContextProvider value={{ readOnly: false }}>
        <XmlFormEditor
          snapshot={snapshotWithHiddenVerbsDiagnostic(true)}
          selectedDefNodeId={1}
          onSelectDef={vi.fn()}
          formApi={makeFormApi(fieldsWithVerbsRevealed())}
          formViews={makeFormViewsController({ effectiveHidden: new Set() })}
        />
      </XmlEditorContextProvider>,
    );

    // No section wraps a top-level `verbs` field, so `ObjectListEditor`'s root container itself
    // is the DOM anchor - it must exist (proving `id`/`tabIndex` actually reached it) and must
    // be the element that ends up focused, not the (by now unmounted, since the summary/reveal
    // row no longer has anything to report) "Reveal fields with issues" button.
    const objectListContainer = document.getElementById("field-verbs");
    expect(objectListContainer).not.toBeNull();
    expect(objectListContainer!.tagName).toBe("DIV");
    expect(document.activeElement).toBe(objectListContainer);
  });
});
