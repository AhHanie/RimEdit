import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { XmlEditorPane } from "./XmlEditorPane";
import { useXmlEditorSession } from "../../hooks/useXmlEditorSession";
import type { UseXmlEditorSessionReturn, XmlEditorFileRef } from "../../hooks/useXmlEditorSession";
import { useXmlFormController } from "../../hooks/useXmlFormController";
import type { XmlFormApi } from "../../hooks/useXmlFormController";
import { FormFieldStore } from "../../lib/formFieldStore";
import type { SchemaCatalog } from "../../../schema-catalog/types";

// Covers issue 08's "Preview button appears for selected Def" and "Preview command is called
// with selected Def identity" acceptance criteria at the XmlEditorPane wiring layer.
// PatchPreviewDialog's own behavior (fetch/disable/reorder/reset) is covered by its own test file.
vi.mock("../../hooks/useXmlEditorSession", () => ({ useXmlEditorSession: vi.fn() }));
vi.mock("../../hooks/useXmlFormController", () => ({ useXmlFormController: vi.fn() }));
vi.mock("../XmlFormEditor/XmlFormEditor", () => ({
  XmlFormEditor: () => <div data-testid="xml-form-editor-stub" />,
}));
vi.mock("../../../patches-editor", () => ({
  PatchEditorPane: () => <div data-testid="patch-editor-pane-stub" />,
  PatchPreviewDialog: (props: {
    projectId: string;
    target: { locationId: string; relativePath: string; defType: string; identity: string; ordinal: number };
    onClose: () => void;
  }) => (
    <div data-testid="patch-preview-dialog-stub">
      {props.projectId}:{props.target.locationId}:{props.target.relativePath}:{props.target.defType}:
      {props.target.identity}:{props.target.ordinal}
    </div>
  ),
}));

const useXmlEditorSessionMock = vi.mocked(useXmlEditorSession);
const useXmlFormControllerMock = vi.mocked(useXmlFormController);

function makeFileRef(overrides: Partial<XmlEditorFileRef> = {}): XmlEditorFileRef {
  return {
    locationId: "proj1",
    sourceKind: "project",
    readOnly: false,
    relativePath: "Defs/Things.xml",
    ...overrides,
  };
}

function makeCatalog(): SchemaCatalog {
  return { formatVersion: 1, packs: [], objectTypes: {}, defTypes: {} };
}

function makeSession(overrides: Partial<UseXmlEditorSessionReturn> = {}): UseXmlEditorSessionReturn {
  return {
    projectId: "proj1",
    relativePath: "Defs/Things.xml",
    readOnly: false,
    baseRawXml: "<Defs></Defs>",
    currentRawXml: "<Defs></Defs>",
    currentParseDiagnostics: [],
    currentValidationDiagnostics: [],
    isBufferValid: true,
    lastValidSnapshot: {
      rawXml: "<Defs></Defs>",
      parsed: {
        nodeCount: 1,
        rootElement: "Defs",
        profile: "defs",
        about: null,
        defs: [
          {
            nodeId: 1,
            defType: "ThingDef",
            defName: "Wall",
            label: "wall",
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
    },
    mode: "form",
    dirty: false,
    canUndo: false,
    canRedo: false,
    savePreview: null,
    saveError: null,
    saveBusy: false,
    loading: false,
    loadError: null,
    applyFormEdit: vi.fn(),
    applyFormEdits: vi.fn(),
    insertDefFromTemplate: vi.fn(),
    insertDefFromUserTemplate: vi.fn(),
    insertDefFromIndexedDef: vi.fn(),
    saveSelectedDefAsTemplate: vi.fn(),
    listUserDefTemplates: vi.fn().mockResolvedValue([]),
    deleteUserDefTemplate: vi.fn(),
    updateRawXml: vi.fn(),
    switchMode: vi.fn(),
    undo: vi.fn(),
    redo: vi.fn(),
    selectDef: vi.fn(),
    requestSavePreview: vi.fn(),
    loadFullSavePreview: vi.fn(),
    confirmSave: vi.fn(),
    clearSavePreview: vi.fn(),
    savePreviewTraceId: null,
    savePreviewStartedAt: null,
    ...overrides,
  };
}

function makeFormApi(): XmlFormApi {
  const store = new FormFieldStore();
  const flushAll = vi.fn().mockResolvedValue(null);
  const flushField = vi.fn().mockResolvedValue(null);
  const base: XmlFormApi = {
    snapshot: null,
    store,
    actions: {
      setFieldValue: vi.fn(),
      focusField: vi.fn(),
      blurField: vi.fn(),
      resetField: vi.fn(),
      clearField: vi.fn(),
      discardDrafts: vi.fn(),
      flushField,
      flushAll,
    },
    hasDraftChanges: false,
    hasPendingCommits: false,
    hasBlockingErrors: false,
    formError: null,
    setFieldValue: vi.fn(),
    focusField: vi.fn(),
    blurField: vi.fn(),
    resetField: vi.fn(),
    clearField: vi.fn(),
    discardDrafts: vi.fn(),
    flushField,
    flushAll,
  };
  return base;
}

describe("XmlEditorPane - patch preview wiring", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("enables the Preview Patches action when a Def is selected", () => {
    useXmlFormControllerMock.mockReturnValue(makeFormApi());
    useXmlEditorSessionMock.mockReturnValue(makeSession());

    render(
      <XmlEditorPane projectId="proj1" file={makeFileRef()} catalog={makeCatalog()} hasOpenTabs={true} />,
    );

    const button = screen.getByLabelText("Preview Patches") as HTMLButtonElement;
    expect(button.disabled).toBe(false);
  });

  it("disables the Preview Patches action when no Def is selected", () => {
    useXmlFormControllerMock.mockReturnValue(makeFormApi());
    useXmlEditorSessionMock.mockReturnValue(
      makeSession({
        lastValidSnapshot: {
          rawXml: "<Defs></Defs>",
          parsed: { nodeCount: 1, rootElement: "Defs", profile: "defs", about: null, defs: [] },
          parseDiagnostics: [],
          validationDiagnostics: [],
          selectedDefNodeId: null,
        },
      }),
    );

    render(
      <XmlEditorPane projectId="proj1" file={makeFileRef()} catalog={makeCatalog()} hasOpenTabs={true} />,
    );

    const button = screen.getByLabelText("Preview Patches") as HTMLButtonElement;
    expect(button.disabled).toBe(true);
  });

  it("opens the patch preview dialog for the selected Def's identity", async () => {
    useXmlFormControllerMock.mockReturnValue(makeFormApi());
    useXmlEditorSessionMock.mockReturnValue(makeSession());

    render(
      <XmlEditorPane projectId="proj1" file={makeFileRef()} catalog={makeCatalog()} hasOpenTabs={true} />,
    );

    expect(screen.queryByTestId("patch-preview-dialog-stub")).toBeNull();
    await userEvent.click(screen.getByLabelText("Preview Patches"));

    expect(screen.getByTestId("patch-preview-dialog-stub").textContent).toBe(
      "proj1:proj1:Defs/Things.xml:ThingDef:Wall:0",
    );
  });

  it("falls back to the Name attribute for an abstract Def with no defName", async () => {
    useXmlFormControllerMock.mockReturnValue(makeFormApi());
    useXmlEditorSessionMock.mockReturnValue(
      makeSession({
        lastValidSnapshot: {
          rawXml: "<Defs></Defs>",
          parsed: {
            nodeCount: 1,
            rootElement: "Defs",
            profile: "defs",
            about: null,
            defs: [
              {
                nodeId: 1,
                defType: "ThingDef",
                defName: null,
                label: null,
                parentName: null,
                line: null,
                column: null,
                attributes: [{ name: "Name", value: "BaseThing", known: true }, { name: "Abstract", value: "True", known: true }],
                children: [],
              },
            ],
          },
          parseDiagnostics: [],
          validationDiagnostics: [],
          selectedDefNodeId: 1,
        },
      }),
    );

    render(
      <XmlEditorPane projectId="proj1" file={makeFileRef()} catalog={makeCatalog()} hasOpenTabs={true} />,
    );

    const button = screen.getByLabelText("Preview Patches") as HTMLButtonElement;
    expect(button.disabled).toBe(false);

    await userEvent.click(button);
    expect(screen.getByTestId("patch-preview-dialog-stub").textContent).toBe(
      "proj1:proj1:Defs/Things.xml:ThingDef:BaseThing:0",
    );
  });

  it("disables Preview Patches when the selected Def has neither defName nor a Name attribute", () => {
    useXmlFormControllerMock.mockReturnValue(makeFormApi());
    useXmlEditorSessionMock.mockReturnValue(
      makeSession({
        lastValidSnapshot: {
          rawXml: "<Defs></Defs>",
          parsed: {
            nodeCount: 1,
            rootElement: "Defs",
            profile: "defs",
            about: null,
            defs: [
              {
                nodeId: 1,
                defType: "ThingDef",
                defName: null,
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
        },
      }),
    );

    render(
      <XmlEditorPane projectId="proj1" file={makeFileRef()} catalog={makeCatalog()} hasOpenTabs={true} />,
    );

    const button = screen.getByLabelText("Preview Patches") as HTMLButtonElement;
    expect(button.disabled).toBe(true);
  });

  it("opens the preview for a read-only source file, keeping the target's origin separate from the active project", async () => {
    useXmlFormControllerMock.mockReturnValue(makeFormApi());
    useXmlEditorSessionMock.mockReturnValue(
      makeSession({
        readOnly: true,
        relativePath: "Defs/Other.xml",
        lastValidSnapshot: {
          rawXml: "<Defs></Defs>",
          parsed: {
            nodeCount: 2,
            rootElement: "Defs",
            profile: "defs",
            about: null,
            defs: [
              {
                nodeId: 1,
                defType: "ThingDef",
                defName: "Floor",
                label: "floor",
                parentName: null,
                line: null,
                column: null,
                attributes: [],
                children: [],
              },
              {
                nodeId: 2,
                defType: "ThingDef",
                defName: "Wall",
                label: "wall",
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
          selectedDefNodeId: 2,
        },
      }),
    );

    render(
      <XmlEditorPane
        projectId="proj1"
        file={makeFileRef({
          locationId: "src1",
          sourceKind: "source",
          readOnly: true,
          relativePath: "Defs/Other.xml",
        })}
        catalog={makeCatalog()}
        hasOpenTabs={true}
      />,
    );

    // Preview must stay enabled for a read-only file, and the dialog must receive the source
    // location/file/ordinal separately from the active editable project ID.
    const button = screen.getByLabelText("Preview Patches") as HTMLButtonElement;
    expect(button.disabled).toBe(false);

    await userEvent.click(button);
    expect(screen.getByTestId("patch-preview-dialog-stub").textContent).toBe(
      "proj1:src1:Defs/Other.xml:ThingDef:Wall:1",
    );
  });
});
