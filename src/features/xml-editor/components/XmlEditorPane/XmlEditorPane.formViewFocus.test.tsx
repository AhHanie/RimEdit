import { screen, waitFor } from "@testing-library/react";
import { renderWithI18n as render } from "../../../../i18n/testing/renderWithI18n";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { XmlEditorPane } from "./XmlEditorPane";
import { useXmlEditorSession } from "../../hooks/useXmlEditorSession";
import type { UseXmlEditorSessionReturn } from "../../hooks/useXmlEditorSession";
import type { SchemaCatalog } from "../../../schema-catalog/types";

// Only the (heavy, Tauri-backed) session hook is mocked here -- `useXmlFormController`,
// `useFormViews`, and `XmlFormEditor`/`FormViewSelector` all run for real, because this test
// exists specifically to prove the real wiring between them: that `XmlEditorPane` actually
// supplies `onFocusedFieldHidden` to `useXmlFormController` (issue 05's signal, issue 06's
// consumer) and that it moves focus to the Form View selector rather than losing it.
vi.mock("../../hooks/useXmlEditorSession", () => ({
  useXmlEditorSession: vi.fn(),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const useXmlEditorSessionMock = vi.mocked(useXmlEditorSession);
const invokeMock = vi.mocked(invoke);

function makeCatalog(): SchemaCatalog {
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
        // Not recommended -- so the synchronous initial resolution (before the persisted
        // preference below resolves) is Default, keeping "description" visible/focusable at
        // first. The persisted preference (resolved later, asynchronously) is what actually
        // switches to this view while the user is still focused on the field it hides.
        formViews: {
          minimal: {
            id: "minimal",
            label: "Minimal",
            order: 10,
            recommended: false,
            hiddenFieldIds: ["description"],
            declaredOnDefType: "ThingDef",
          },
        },
      },
    },
  };
}

function makeSession(): UseXmlEditorSessionReturn {
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
    applyFormEdits: vi.fn().mockResolvedValue("<Defs></Defs>"),
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

describe("XmlEditorPane - Form View focus redirect (issue 05/06 wiring)", () => {
  it("moves focus to the Form View selector when a background view switch hides the currently-focused field's root", async () => {
    useXmlEditorSessionMock.mockReturnValue(makeSession());

    let resolveLastSelected!: (value: {
      selected: { origin: string; id: string } | null;
      warning: null;
    }) => void;
    const lastSelectedPromise = new Promise<{
      selected: { origin: string; id: string } | null;
      warning: null;
    }>((resolve) => {
      resolveLastSelected = resolve;
    });

    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "list_custom_form_views") return Promise.resolve({ views: [], warning: null });
      if (cmd === "get_last_selected_form_view") return lastSelectedPromise;
      if (cmd === "set_last_selected_form_view") return Promise.resolve(undefined);
      return Promise.reject(new Error(`unexpected invoke: ${cmd}`));
    });

    render(
      <XmlEditorPane
        projectId="proj1"
        file={{
          locationId: "proj1",
          sourceKind: "project",
          readOnly: false,
          relativePath: "Defs/Things.xml",
        }}
        catalog={makeCatalog()}
        gameVersion="1.6"
        hasOpenTabs
      />,
    );

    // Initially Default is selected (the persisted preference hasn't resolved yet), so
    // "description" is visible and focusable.
    const descriptionInput = await screen.findByLabelText("Description");
    await userEvent.click(descriptionInput);
    expect(document.activeElement).toBe(descriptionInput);

    // Now the real persisted preference resolves, asynchronously and without any further user
    // interaction -- switching to the "minimal" schema view, which hides "description" out from
    // under the still-focused field.
    resolveLastSelected({ selected: { origin: "schema", id: "minimal" }, warning: null });

    await waitFor(() => expect(screen.queryByLabelText("Description")).toBeNull());

    // Focus must land on the Form View selector's `<select>`, not fall back to `document.body`.
    const viewSelect = screen.getByLabelText("View");
    await waitFor(() => expect(document.activeElement).toBe(viewSelect));
    expect(document.activeElement).not.toBe(document.body);
  });
});
