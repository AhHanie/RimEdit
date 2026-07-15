import { screen } from "@testing-library/react";
import { renderWithI18n as render } from "../../../../i18n/testing/renderWithI18n";
import { XmlEditorPane } from "./XmlEditorPane";
import { useXmlEditorSession } from "../../hooks/useXmlEditorSession";
import type { UseXmlEditorSessionReturn, XmlEditorFileRef } from "../../hooks/useXmlEditorSession";
import { useXmlFormController } from "../../hooks/useXmlFormController";
import type { XmlFormApi } from "../../hooks/useXmlFormController";
import { FormFieldStore } from "../../lib/formFieldStore";
import { FORM_VIEW_SELECTOR_SELECT_ID } from "../../../form-views/components/FormViewSelector/FormViewSelector";
import type { SchemaCatalog } from "../../../schema-catalog/types";

// Covers issue 04's "patch file route detection" and "switch raw/form mode" acceptance
// criteria at the XmlEditorPane routing layer. PatchEditorPane's own behavior (parsing,
// editing, add operation, etc.) is covered separately by its own test file.
vi.mock("../../hooks/useXmlEditorSession", () => ({ useXmlEditorSession: vi.fn() }));
vi.mock("../../hooks/useXmlFormController", () => ({ useXmlFormController: vi.fn() }));
vi.mock("../XmlFormEditor/XmlFormEditor", () => ({
  XmlFormEditor: () => <div data-testid="xml-form-editor-stub" />,
}));
vi.mock("../../../patches-editor", () => ({
  PatchEditorPane: (props: { relativePath: string; registerFlush?: (flush: () => Promise<void>) => void }) => {
    props.registerFlush?.(async () => undefined);
    return <div data-testid="patch-editor-pane-stub">{props.relativePath}</div>;
  },
  PatchPreviewDialog: () => <div data-testid="patch-preview-dialog-stub" />,
}));

const useXmlEditorSessionMock = vi.mocked(useXmlEditorSession);
const useXmlFormControllerMock = vi.mocked(useXmlFormController);

function makeFileRef(overrides: Partial<XmlEditorFileRef> = {}): XmlEditorFileRef {
  return {
    locationId: "proj1",
    sourceKind: "project",
    readOnly: false,
    relativePath: "Patches/MyPatch.xml",
    ...overrides,
  };
}

function makeCatalog(): SchemaCatalog {
  return { formatVersion: 1, packs: [], objectTypes: {}, defTypes: {} };
}

function makeSession(overrides: Partial<UseXmlEditorSessionReturn> = {}): UseXmlEditorSessionReturn {
  return {
    projectId: "proj1",
    relativePath: "Patches/MyPatch.xml",
    readOnly: false,
    baseRawXml: "<Patch></Patch>",
    currentRawXml: "<Patch></Patch>",
    currentParseDiagnostics: [],
    currentValidationDiagnostics: [],
    isBufferValid: true,
    lastValidSnapshot: {
      rawXml: "<Patch></Patch>",
      parsed: { nodeCount: 1, rootElement: "Patch", profile: "patch", about: null, defs: [] },
      parseDiagnostics: [],
      validationDiagnostics: [],
      selectedDefNodeId: null,
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

describe("XmlEditorPane - patch file routing", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("routes a <Patch>-rooted file to PatchEditorPane in form mode", () => {
    useXmlFormControllerMock.mockReturnValue(makeFormApi());
    useXmlEditorSessionMock.mockReturnValue(makeSession());

    render(
      <XmlEditorPane projectId="proj1" file={makeFileRef()} catalog={makeCatalog()} hasOpenTabs={true} />,
    );

    expect(screen.getByTestId("patch-editor-pane-stub")).toBeTruthy();
    expect(screen.queryByTestId("xml-form-editor-stub")).toBeNull();
  });

  it("still routes a <Defs>-rooted file to XmlFormEditor in form mode", () => {
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
      <XmlEditorPane
        projectId="proj1"
        file={makeFileRef({ relativePath: "Defs/Things.xml" })}
        catalog={makeCatalog()}
        hasOpenTabs={true}
      />,
    );

    expect(screen.getByTestId("xml-form-editor-stub")).toBeTruthy();
    expect(screen.queryByTestId("patch-editor-pane-stub")).toBeNull();
  });

  it("switching to raw mode shows the raw editor instead of the patch editor pane", () => {
    useXmlFormControllerMock.mockReturnValue(makeFormApi());
    useXmlEditorSessionMock.mockReturnValue(makeSession({ mode: "raw" }));

    render(
      <XmlEditorPane projectId="proj1" file={makeFileRef()} catalog={makeCatalog()} hasOpenTabs={true} />,
    );

    expect(screen.queryByTestId("patch-editor-pane-stub")).toBeNull();
    expect(screen.queryByTestId("xml-form-editor-stub")).toBeNull();
  });

  it("disables the New Def action for patch files", () => {
    useXmlFormControllerMock.mockReturnValue(makeFormApi());
    useXmlEditorSessionMock.mockReturnValue(makeSession());

    render(
      <XmlEditorPane projectId="proj1" file={makeFileRef()} catalog={makeCatalog()} hasOpenTabs={true} />,
    );

    const button = screen.getByLabelText("New Def") as HTMLButtonElement;
    expect(button.disabled).toBe(true);
  });

  // Issue 09 (Plan.md section 11): Def `formViews` never apply to Patch mode -- it's an
  // operation-tree editor, not `XmlFormEditor`, so `XmlFormEditor` (and the Form View selector
  // it owns) must never mount for a `<Patch>`-rooted file. `XmlFormEditor` is stubbed above, so
  // this also guards against a future change accidentally rendering the real
  // `FormViewSelector` outside of `XmlFormEditor`.
  it("never shows the Form View selector for a Patch profile", () => {
    useXmlFormControllerMock.mockReturnValue(makeFormApi());
    useXmlEditorSessionMock.mockReturnValue(makeSession());

    render(
      <XmlEditorPane projectId="proj1" file={makeFileRef()} catalog={makeCatalog()} hasOpenTabs={true} />,
    );

    expect(screen.queryByLabelText("View")).toBeNull();
    expect(document.getElementById(FORM_VIEW_SELECTOR_SELECT_ID)).toBeNull();
  });
});
