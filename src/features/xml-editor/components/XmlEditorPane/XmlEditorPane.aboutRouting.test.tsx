import { render, screen } from "@testing-library/react";
import { XmlEditorPane } from "./XmlEditorPane";
import { useXmlEditorSession } from "../../hooks/useXmlEditorSession";
import type { UseXmlEditorSessionReturn, XmlEditorFileRef } from "../../hooks/useXmlEditorSession";
import { useXmlFormController } from "../../hooks/useXmlFormController";
import type { XmlFormApi } from "../../hooks/useXmlFormController";
import { FormFieldStore } from "../../lib/formFieldStore";
import type { AboutMetadataView } from "../../types/xmlDocument";
import { FORM_VIEW_SELECTOR_SELECT_ID } from "../../../form-views/components/FormViewSelector/FormViewSelector";
import type { SchemaCatalog } from "../../../schema-catalog/types";

// Covers the About.xml UI editor plan's "profile routing" and "read-only source"
// acceptance criteria at the XmlEditorPane routing layer -- AboutEditorPane's own
// section behavior (identity/dependency/etc. editing) is covered by its own test file.
vi.mock("../../hooks/useXmlEditorSession", () => ({ useXmlEditorSession: vi.fn() }));
vi.mock("../../hooks/useXmlFormController", () => ({ useXmlFormController: vi.fn() }));
vi.mock("../XmlFormEditor/XmlFormEditor", () => ({
  XmlFormEditor: () => <div data-testid="xml-form-editor-stub" />,
}));
vi.mock("../../../patches-editor", () => ({
  PatchEditorPane: () => <div data-testid="patch-editor-pane-stub" />,
  PatchPreviewDialog: () => <div data-testid="patch-preview-dialog-stub" />,
}));
vi.mock("../../../about-editor", () => ({
  AboutEditorPane: (props: { readOnly: boolean; registerFlush?: (flush: () => Promise<void>) => void }) => {
    props.registerFlush?.(async () => undefined);
    return <div data-testid="about-editor-pane-stub">{props.readOnly ? "read-only" : "editable"}</div>;
  },
}));

const useXmlEditorSessionMock = vi.mocked(useXmlEditorSession);
const useXmlFormControllerMock = vi.mocked(useXmlFormController);

function makeAboutView(): AboutMetadataView {
  return {
    rootNodeId: 1,
    fields: {
      packageId: { value: "foo.bar" },
      name: { value: "Foo" },
      shortName: { value: null },
      author: { value: null },
      authors: { items: [], present: false },
      modIconPath: { value: null },
      modVersion: { value: null },
      url: { value: null },
      description: { value: null },
      steamAppId: { value: null },
      targetVersion: { value: null },
      supportedVersions: { items: ["1.6"], present: true },
      loadBefore: { items: [], present: false },
      loadAfter: { items: [], present: false },
      forceLoadBefore: { items: [], present: false },
      forceLoadAfter: { items: [], present: false },
      incompatibleWith: { items: [], present: false },
      modDependencies: [],
      descriptionsByVersion: [],
      modDependenciesByVersion: [],
      loadBeforeByVersion: [],
      loadAfterByVersion: [],
      incompatibleWithByVersion: [],
    },
    unknownChildren: [],
  };
}

function makeFileRef(overrides: Partial<XmlEditorFileRef> = {}): XmlEditorFileRef {
  return {
    locationId: "proj1",
    sourceKind: "project",
    readOnly: false,
    relativePath: "About/About.xml",
    ...overrides,
  };
}

function makeCatalog(): SchemaCatalog {
  return { formatVersion: 1, packs: [], objectTypes: {}, defTypes: {} };
}

function makeSession(overrides: Partial<UseXmlEditorSessionReturn> = {}): UseXmlEditorSessionReturn {
  const rawXml = "<ModMetaData><packageId>foo.bar</packageId></ModMetaData>";
  return {
    projectId: "proj1",
    relativePath: "About/About.xml",
    readOnly: false,
    baseRawXml: rawXml,
    currentRawXml: rawXml,
    currentParseDiagnostics: [],
    currentValidationDiagnostics: [],
    isBufferValid: true,
    lastValidSnapshot: {
      rawXml,
      parsed: {
        nodeCount: 2,
        rootElement: "ModMetaData",
        profile: "about",
        about: makeAboutView(),
        defs: [],
      },
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

describe("XmlEditorPane - About.xml routing", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("routes a ModMetaData-rooted file to AboutEditorPane in form mode", () => {
    useXmlFormControllerMock.mockReturnValue(makeFormApi());
    useXmlEditorSessionMock.mockReturnValue(makeSession());

    render(<XmlEditorPane projectId="proj1" file={makeFileRef()} catalog={makeCatalog()} hasOpenTabs={true} />);

    expect(screen.getByTestId("about-editor-pane-stub")).toBeTruthy();
    expect(screen.queryByTestId("xml-form-editor-stub")).toBeNull();
    expect(screen.queryByTestId("patch-editor-pane-stub")).toBeNull();
  });

  it("switching to raw mode shows the raw editor instead of the About editor pane", () => {
    useXmlFormControllerMock.mockReturnValue(makeFormApi());
    useXmlEditorSessionMock.mockReturnValue(makeSession({ mode: "raw" }));

    render(<XmlEditorPane projectId="proj1" file={makeFileRef()} catalog={makeCatalog()} hasOpenTabs={true} />);

    expect(screen.queryByTestId("about-editor-pane-stub")).toBeNull();
    expect(screen.queryByTestId("xml-form-editor-stub")).toBeNull();
  });

  it("renders a read-only About editor for a read-only source file", () => {
    useXmlFormControllerMock.mockReturnValue(makeFormApi());
    useXmlEditorSessionMock.mockReturnValue(makeSession({ readOnly: true }));

    render(
      <XmlEditorPane
        projectId="proj1"
        file={makeFileRef({ readOnly: true, sourceKind: "source", locationName: "RimWorld Core" })}
        catalog={makeCatalog()}
        hasOpenTabs={true}
      />,
    );

    expect(screen.getByTestId("about-editor-pane-stub").textContent).toBe("read-only");
  });

  it("disables the New Def, Save as Template, and Preview Patches actions for About.xml", () => {
    useXmlFormControllerMock.mockReturnValue(makeFormApi());
    useXmlEditorSessionMock.mockReturnValue(makeSession());

    render(<XmlEditorPane projectId="proj1" file={makeFileRef()} catalog={makeCatalog()} hasOpenTabs={true} />);

    expect((screen.getByLabelText("New Def") as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByLabelText("Save as Template") as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByLabelText("Preview Patches") as HTMLButtonElement).disabled).toBe(true);
  });

  it("still routes a <Defs>-rooted file to XmlFormEditor, not the About editor", () => {
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
    expect(screen.queryByTestId("about-editor-pane-stub")).toBeNull();
  });

  // Issue 09 (Plan.md section 11): Def `formViews` never apply to About.xml -- `XmlFormEditor`
  // (and the Form View selector it owns) must never mount for a `<ModMetaData>`-rooted file.
  // `XmlFormEditor` is stubbed above, so this also guards against a future change accidentally
  // rendering the real `FormViewSelector` outside of `XmlFormEditor`.
  it("never shows the Form View selector for an About profile", () => {
    useXmlFormControllerMock.mockReturnValue(makeFormApi());
    useXmlEditorSessionMock.mockReturnValue(makeSession());

    render(<XmlEditorPane projectId="proj1" file={makeFileRef()} catalog={makeCatalog()} hasOpenTabs={true} />);

    expect(screen.queryByLabelText("View")).toBeNull();
    expect(document.getElementById(FORM_VIEW_SELECTOR_SELECT_ID)).toBeNull();
  });
});
