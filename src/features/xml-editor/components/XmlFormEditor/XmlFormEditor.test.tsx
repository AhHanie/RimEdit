import { render, screen } from "@testing-library/react";
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
