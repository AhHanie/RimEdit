import { render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { XmlEditorPane } from "./XmlEditorPane";
import { useXmlEditorSession } from "../../hooks/useXmlEditorSession";
import type { UseXmlEditorSessionReturn, XmlEditorFileRef } from "../../hooks/useXmlEditorSession";
import { FORM_VIEW_SELECTOR_SELECT_ID } from "../../../form-views/components/FormViewSelector/FormViewSelector";
import type { SchemaCatalog } from "../../../schema-catalog/types";

// Issue 09 (Plan.md section 11): Form Views apply only when `profile === "defs"` AND the
// selected Def resolves a schema. This file proves that gate for Defs-profile documents at
// the `XmlEditorPane` integration boundary, running the exact same unmocked
// `useFormViews`/`useXmlFormController`/`XmlFormEditor` wiring as
// `XmlEditorPane.formViewFocus.test.tsx` -- so a regression in the
// `documentProfileEarly === "defs"` check (in `XmlEditorPane`) or in `useFormViews`'s
// `applicable` computation (schema resolvability) would be caught here, not just hidden
// behind a mocked stub. Patch/About routing (which never reaches `XmlFormEditor` at all) is
// covered by the additional assertions appended to `XmlEditorPane.patchRouting.test.tsx` and
// `XmlEditorPane.aboutRouting.test.tsx`.
vi.mock("../../hooks/useXmlEditorSession", () => ({ useXmlEditorSession: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const useXmlEditorSessionMock = vi.mocked(useXmlEditorSession);
const invokeMock = vi.mocked(invoke);

function makeFileRef(overrides: Partial<XmlEditorFileRef> = {}): XmlEditorFileRef {
  return {
    locationId: "proj1",
    sourceKind: "project",
    readOnly: false,
    relativePath: "Defs/Things.xml",
    ...overrides,
  };
}

function makeCatalogWithThingDef(): SchemaCatalog {
  return {
    formatVersion: 1,
    packs: [],
    objectTypes: {},
    defTypes: {
      ThingDef: {
        inherits: [],
        abstractType: false,
        fieldOrder: ["defName", "description"],
        fields: {
          defName: {
            type: { kind: "string" },
            required: true,
            label: "Def Name",
            examples: [],
            repeatable: false,
            xml: "element",
            flags: false,
          },
          description: {
            type: { kind: "string" },
            required: false,
            label: "Description",
            examples: [],
            repeatable: false,
            xml: "element",
            flags: false,
          },
        },
      },
    },
  };
}

function makeEmptyCatalog(): SchemaCatalog {
  return { formatVersion: 1, packs: [], objectTypes: {}, defTypes: {} };
}

function baseSessionFields() {
  return {
    projectId: "proj1",
    readOnly: false,
    isBufferValid: true,
    mode: "form" as const,
    dirty: false,
    canUndo: false,
    canRedo: false,
    savePreview: null,
    saveError: null,
    saveBusy: false,
    loading: false,
    loadError: null,
    applyFormEdit: vi.fn(),
    applyFormEdits: vi.fn().mockResolvedValue(""),
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
  };
}

describe("XmlEditorPane - Form View profile gating (issue 09, real wiring)", () => {
  beforeEach(() => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "list_custom_form_views") return Promise.resolve({ views: [], warning: null });
      if (cmd === "get_last_selected_form_view") return Promise.resolve({ selected: null, warning: null });
      if (cmd === "set_last_selected_form_view") return Promise.resolve(undefined);
      return Promise.reject(new Error(`unexpected invoke: ${cmd}`));
    });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("shows the Form View selector for a Defs profile with a resolvable schema", async () => {
    useXmlEditorSessionMock.mockReturnValue({
      ...baseSessionFields(),
      relativePath: "Defs/Things.xml",
      baseRawXml: "<Defs></Defs>",
      currentRawXml: "<Defs></Defs>",
      currentParseDiagnostics: [],
      currentValidationDiagnostics: [],
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
    } as UseXmlEditorSessionReturn);

    render(
      <XmlEditorPane
        projectId="proj1"
        file={makeFileRef()}
        catalog={makeCatalogWithThingDef()}
        gameVersion="1.6"
        hasOpenTabs
      />,
    );

    expect(await screen.findByLabelText("View")).toBeTruthy();
    expect(document.getElementById(FORM_VIEW_SELECTOR_SELECT_ID)).not.toBeNull();
  });

  it("shows no Form View controls for a Defs profile whose selected Def has no resolvable schema", async () => {
    useXmlEditorSessionMock.mockReturnValue({
      ...baseSessionFields(),
      relativePath: "Defs/Things.xml",
      baseRawXml: "<Defs></Defs>",
      currentRawXml: "<Defs></Defs>",
      currentParseDiagnostics: [],
      currentValidationDiagnostics: [],
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
              // No entry for "MysteryDef" exists in the empty catalog used below.
              defType: "MysteryDef",
              defName: "Mystery_1",
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
    } as UseXmlEditorSessionReturn);

    render(
      <XmlEditorPane
        projectId="proj1"
        file={makeFileRef()}
        catalog={makeEmptyCatalog()}
        gameVersion="1.6"
        hasOpenTabs
      />,
    );

    // Let pending effects (e.g. the async custom-view fetch) settle before asserting a
    // negative, so state updates from those effects are captured inside `waitFor`'s `act`.
    await waitFor(() => {
      expect(screen.queryByLabelText("View")).toBeNull();
      expect(document.getElementById(FORM_VIEW_SELECTOR_SELECT_ID)).toBeNull();
    });
  });

  it("shows no Form View controls for a generic/raw-XML profile even if switched to form mode", async () => {
    useXmlEditorSessionMock.mockReturnValue({
      ...baseSessionFields(),
      relativePath: "Misc/Config.xml",
      baseRawXml: "<Config></Config>",
      currentRawXml: "<Config></Config>",
      currentParseDiagnostics: [],
      currentValidationDiagnostics: [],
      // `mode: "form"` even though a genericXml profile normally defaults to raw mode -- this
      // directly exercises the `documentProfileEarly === "defs"` guard on the `selectedDef`
      // passed into `useFormViews`, independent of how "form" mode was reached.
      lastValidSnapshot: {
        rawXml: "<Config></Config>",
        parsed: {
          nodeCount: 1,
          rootElement: "Config",
          profile: "genericXml",
          about: null,
          defs: [],
        },
        parseDiagnostics: [],
        validationDiagnostics: [],
        selectedDefNodeId: null,
      },
    } as UseXmlEditorSessionReturn);

    render(
      <XmlEditorPane
        projectId="proj1"
        file={makeFileRef({ relativePath: "Misc/Config.xml" })}
        catalog={makeCatalogWithThingDef()}
        gameVersion="1.6"
        hasOpenTabs
      />,
    );

    await Promise.resolve();
    expect(screen.queryByLabelText("View")).toBeNull();
    expect(document.getElementById(FORM_VIEW_SELECTOR_SELECT_ID)).toBeNull();
  });
});
