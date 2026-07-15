import { screen, waitFor } from "@testing-library/react";
import { renderWithI18n as render } from "../../../../i18n/testing/renderWithI18n";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { XmlEditorPane } from "./XmlEditorPane";
import { useXmlEditorSession } from "../../hooks/useXmlEditorSession";
import type {
  UseXmlEditorSessionReturn,
  XmlEditorFileRef,
} from "../../hooks/useXmlEditorSession";
import { useXmlFormController } from "../../hooks/useXmlFormController";
import type { XmlFormApi } from "../../hooks/useXmlFormController";
import { FormFieldStore } from "../../lib/formFieldStore";
import type { SchemaCatalog } from "../../../schema-catalog/types";
import type { UserDefTemplate, UserDefTemplateSummary } from "../../types/defTemplates";
import type { CreateDefResult } from "../../types/createDef";
import type { IndexedDefSearchResult } from "../../../def-index";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

function makeIndexedSearchResult(): IndexedDefSearchResult {
  return {
    rank: 1,
    def: {
      key: { defType: "ThingDef", defName: "Gun_Autopistol" },
      defType: "ThingDef",
      defName: "Gun_Autopistol",
      label: "autopistol",
      relativePath: "Defs/Weapons.xml",
      nodeId: 7,
      source: {
        locationId: "core1",
        locationName: "Core",
        sourceKind: "source",
        sourceType: "baseGame",
        readOnly: true,
      },
      fields: [],
    },
  };
}

// XmlEditorPane's "flush pending form drafts before X" wrappers are plain closures
// defined inline in the component - they can only be exercised by rendering the pane
// itself, not by testing XmlEditorToolbar/CreateDefWizard/SaveDefTemplateDialog against
// a hand-built session mock (those tests call session methods directly, bypassing the
// pane's wrapping). XmlFormEditor is stubbed out so no real schema-driven form needs to
// be built; only the flush-before-mutate ordering is under test here.
vi.mock("../../hooks/useXmlEditorSession", () => ({
  useXmlEditorSession: vi.fn(),
}));
vi.mock("../../hooks/useXmlFormController", () => ({
  useXmlFormController: vi.fn(),
}));
vi.mock("../XmlFormEditor/XmlFormEditor", () => ({
  XmlFormEditor: () => <div data-testid="xml-form-editor-stub" />,
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
  return {
    formatVersion: 1,
    packs: [],
    objectTypes: {},
    defTypes: {
      ThingDef: {
        label: "Thing",
        inherits: [],
        abstractType: false,
        fieldOrder: ["defName", "label"],
        fields: {
          defName: { type: { kind: "string" }, required: true, examples: [], repeatable: false, xml: "element", flags: false },
          label: { type: { kind: "localizedString" }, required: false, examples: [], repeatable: false, xml: "element", flags: false },
        },
      },
    },
  };
}

function makeUserTemplateSummary(
  overrides: Partial<UserDefTemplateSummary> = {},
): UserDefTemplateSummary {
  return {
    id: "tpl-1",
    defType: "ThingDef",
    name: "Autopistol base",
    description: null,
    originalDefName: "Gun_Autopistol",
    originalLabel: "autopistol",
    sourceRelativePath: "Defs/Weapons.xml",
    gameVersion: "1.6",
    createdAt: "2026-07-05T00:00:00Z",
    updatedAt: "2026-07-05T00:00:00Z",
    ...overrides,
  };
}

function makeUserTemplate(overrides: Partial<UserDefTemplate> = {}): UserDefTemplate {
  return {
    id: "tpl-1",
    defType: "ThingDef",
    name: "Weapon base",
    description: null,
    xml: "<ThingDef><defName>Gun_Autopistol</defName></ThingDef>",
    originalDefName: "Gun_Autopistol",
    originalLabel: "autopistol",
    sourceRelativePath: "Defs/Weapons.xml",
    gameVersion: "1.6",
    createdAt: "2026-07-05T00:00:00Z",
    updatedAt: "2026-07-05T00:00:00Z",
    ...overrides,
  };
}

function makeCreateDefResult(): CreateDefResult {
  return {
    editorDocument: {
      projectId: "proj1",
      relativePath: "Defs/Things.xml",
      rawXml: "<Defs></Defs>",
      document: { nodeCount: 1, rootElement: "Defs", profile: "defs", about: null, defs: [] },
      parseDiagnostics: [],
      validationDiagnostics: [],
    },
    insertedNodeId: 1,
    insertedDefType: "ThingDef",
    insertedDefName: "Gun_MyPistol",
  };
}

function makeSession(
  overrides: Partial<UseXmlEditorSessionReturn> = {},
): UseXmlEditorSessionReturn {
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
            defName: "Gun_Old",
            label: "old label",
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
    insertDefFromTemplate: vi.fn().mockResolvedValue(makeCreateDefResult()),
    insertDefFromUserTemplate: vi.fn().mockResolvedValue(makeCreateDefResult()),
    insertDefFromIndexedDef: vi.fn().mockResolvedValue(makeCreateDefResult()),
    saveSelectedDefAsTemplate: vi.fn().mockResolvedValue(makeUserTemplate()),
    listUserDefTemplates: vi.fn().mockResolvedValue([]),
    deleteUserDefTemplate: vi.fn().mockResolvedValue(undefined),
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

function makeFormApi(overrides: Partial<XmlFormApi> = {}): XmlFormApi {
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
  return { ...base, ...overrides };
}

describe("XmlEditorPane - flush form drafts before template mutations", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("flushes form drafts before saving the selected Def as a template", async () => {
    const order: string[] = [];
    const formApi = makeFormApi({
      flushAll: vi.fn(async () => {
        order.push("flushAll");
        return null;
      }),
    });
    useXmlFormControllerMock.mockReturnValue(formApi);

    const saveSelectedDefAsTemplate = vi.fn(async (name: string) => {
      order.push(`save:${name}`);
      return makeUserTemplate({ name });
    });
    useXmlEditorSessionMock.mockReturnValue(
      makeSession({ saveSelectedDefAsTemplate }),
    );

    render(
      <XmlEditorPane
        projectId="proj1"
        file={makeFileRef()}
        catalog={makeCatalog()}
        hasOpenTabs={true}
      />,
    );

    await userEvent.click(screen.getByLabelText("Save as Template"));
    const input = await screen.findByLabelText("Template name");
    await userEvent.clear(input);
    await userEvent.type(input, "My template");
    await userEvent.click(screen.getByText("Save"));

    await waitFor(() => {
      expect(saveSelectedDefAsTemplate).toHaveBeenCalledWith("My template");
    });
    expect(order).toEqual(["flushAll", "save:My template"]);
  });

  it("flushes form drafts before inserting a Def from a user template", async () => {
    const order: string[] = [];
    const formApi = makeFormApi({
      flushAll: vi.fn(async () => {
        order.push("flushAll");
        return null;
      }),
    });
    useXmlFormControllerMock.mockReturnValue(formApi);

    const insertDefFromUserTemplate = vi.fn(async (templateId: string, defName: string) => {
      order.push(`insert:${templateId}:${defName}`);
      return makeCreateDefResult();
    });
    useXmlEditorSessionMock.mockReturnValue(
      makeSession({
        insertDefFromUserTemplate,
        listUserDefTemplates: vi.fn().mockResolvedValue([makeUserTemplateSummary()]),
      }),
    );

    render(
      <XmlEditorPane
        projectId="proj1"
        file={makeFileRef()}
        catalog={makeCatalog()}
        hasOpenTabs={true}
      />,
    );

    await userEvent.click(screen.getByLabelText("New Def"));
    await userEvent.click(screen.getByText("Thing"));
    await userEvent.click(await screen.findByText("Autopistol base"));

    const input = screen.getByPlaceholderText("e.g. MyThing");
    await userEvent.type(input, "Gun_MyPistol");
    await userEvent.click(screen.getByText("Create"));

    await waitFor(() => {
      expect(insertDefFromUserTemplate).toHaveBeenCalledWith("tpl-1", "Gun_MyPistol");
    });
    // A successful insert also selects the new Def (`editorSession.selectDef`), which
    // flushes drafts again on its own - so only assert the flush-before-insert ordering,
    // not the exact call sequence.
    const flushIndex = order.indexOf("flushAll");
    const insertIndex = order.indexOf("insert:tpl-1:Gun_MyPistol");
    expect(flushIndex).toBeGreaterThanOrEqual(0);
    expect(insertIndex).toBeGreaterThan(flushIndex);
  });

  it("flushes form drafts before inserting a Def from an indexed def", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "search_defs") return [makeIndexedSearchResult()];
      return undefined;
    });

    const order: string[] = [];
    const formApi = makeFormApi({
      flushAll: vi.fn(async () => {
        order.push("flushAll");
        return null;
      }),
    });
    useXmlFormControllerMock.mockReturnValue(formApi);

    const insertDefFromIndexedDef = vi.fn(async (source: { defName: string }, defName: string) => {
      order.push(`insert:${source.defName}:${defName}`);
      return makeCreateDefResult();
    });
    useXmlEditorSessionMock.mockReturnValue(
      makeSession({ insertDefFromIndexedDef }),
    );

    render(
      <XmlEditorPane
        projectId="proj1"
        file={makeFileRef()}
        catalog={makeCatalog()}
        hasOpenTabs={true}
      />,
    );

    await userEvent.click(screen.getByLabelText("New Def"));
    await userEvent.click(screen.getByText("Thing"));
    await userEvent.click(screen.getByRole("tab", { name: "Indexed Defs" }));
    await userEvent.click(await screen.findByText("autopistol"));

    const input = screen.getByPlaceholderText("e.g. MyThing");
    await userEvent.type(input, "Gun_MyPistol");
    await userEvent.click(screen.getByText("Create"));

    await waitFor(() => {
      expect(insertDefFromIndexedDef).toHaveBeenCalledWith(
        makeIndexedSearchResult().def,
        "Gun_MyPistol",
      );
    });
    const flushIndex = order.indexOf("flushAll");
    const insertIndex = order.indexOf("insert:Gun_Autopistol:Gun_MyPistol");
    expect(flushIndex).toBeGreaterThanOrEqual(0);
    expect(insertIndex).toBeGreaterThan(flushIndex);
  });

  it("does not flush form drafts when in raw mode", async () => {
    const formApi = makeFormApi();
    useXmlFormControllerMock.mockReturnValue(formApi);

    const saveSelectedDefAsTemplate = vi.fn().mockResolvedValue(makeUserTemplate());
    useXmlEditorSessionMock.mockReturnValue(
      makeSession({ mode: "raw", saveSelectedDefAsTemplate }),
    );

    render(
      <XmlEditorPane
        projectId="proj1"
        file={makeFileRef()}
        catalog={makeCatalog()}
        hasOpenTabs={true}
      />,
    );

    await userEvent.click(screen.getByLabelText("Save as Template"));
    const input = await screen.findByLabelText("Template name");
    await userEvent.clear(input);
    await userEvent.type(input, "Raw mode template");
    await userEvent.click(screen.getByText("Save"));

    await waitFor(() => {
      expect(saveSelectedDefAsTemplate).toHaveBeenCalled();
    });
    expect(formApi.flushAll).not.toHaveBeenCalled();
  });
});

describe("XmlEditorPane - canSaveAsTemplate derivation", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("disables Save as Template when no Def is selected", () => {
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
        file={makeFileRef()}
        catalog={makeCatalog()}
        hasOpenTabs={true}
      />,
    );

    const button = screen.getByLabelText("Save as Template") as HTMLButtonElement;
    expect(button.disabled).toBe(true);
  });

  it("disables Save as Template when the selected node id no longer resolves to a Def", () => {
    useXmlFormControllerMock.mockReturnValue(makeFormApi());
    useXmlEditorSessionMock.mockReturnValue(
      makeSession({
        lastValidSnapshot: {
          rawXml: "<Defs></Defs>",
          parsed: { nodeCount: 1, rootElement: "Defs", profile: "defs", about: null, defs: [] },
          parseDiagnostics: [],
          validationDiagnostics: [],
          // Stale id from before a raw-XML re-parse removed the Def it pointed at.
          selectedDefNodeId: 1,
        },
      }),
    );

    render(
      <XmlEditorPane
        projectId="proj1"
        file={makeFileRef()}
        catalog={makeCatalog()}
        hasOpenTabs={true}
      />,
    );

    const button = screen.getByLabelText("Save as Template") as HTMLButtonElement;
    expect(button.disabled).toBe(true);
  });

  it("enables Save as Template once the selected node id resolves to a Def in the current snapshot", () => {
    useXmlFormControllerMock.mockReturnValue(makeFormApi());
    useXmlEditorSessionMock.mockReturnValue(makeSession());

    render(
      <XmlEditorPane
        projectId="proj1"
        file={makeFileRef()}
        catalog={makeCatalog()}
        hasOpenTabs={true}
      />,
    );

    const button = screen.getByLabelText("Save as Template") as HTMLButtonElement;
    expect(button.disabled).toBe(false);
  });
});
