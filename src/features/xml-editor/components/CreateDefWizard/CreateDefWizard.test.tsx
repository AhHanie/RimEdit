import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { confirm } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { CreateDefWizard } from "./CreateDefWizard";
import type { UseXmlEditorSessionReturn } from "../../hooks/useXmlEditorSession";
import type { SchemaCatalog } from "../../../schema-catalog/types";
import type { UserDefTemplateSummary } from "../../types/defTemplates";
import type { CreateDefResult } from "../../types/createDef";
import type { IndexedDefSearchResult } from "../../../def-index";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const confirmMock = vi.mocked(confirm);
const invokeMock = vi.mocked(invoke);

function makeIndexedSearchResult(
  overrides: Partial<IndexedDefSearchResult["def"]> = {},
): IndexedDefSearchResult {
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
      ...overrides,
    },
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
        templates: {
          weapon: {
            id: "weapon",
            label: "Weapon base",
            includeRequiredFields: true,
            promptFields: [],
            fieldValues: {},
          },
        },
      },
      PawnKindDef: {
        label: "Pawn Kind",
        inherits: [],
        abstractType: false,
        fieldOrder: ["defName"],
        fields: {
          defName: { type: { kind: "string" }, required: true, examples: [], repeatable: false, xml: "element", flags: false },
        },
      },
    },
  };
}

function makeUserTemplate(
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
    insertedDefName: "Gun_New",
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
      parsed: { nodeCount: 1, rootElement: "Defs", profile: "defs", about: null, defs: [] },
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
    insertDefFromTemplate: vi.fn().mockResolvedValue(makeCreateDefResult()),
    insertDefFromUserTemplate: vi.fn().mockResolvedValue(makeCreateDefResult()),
    insertDefFromIndexedDef: vi.fn().mockResolvedValue(makeCreateDefResult()),
    saveSelectedDefAsTemplate: vi.fn(),
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

describe("CreateDefWizard - user templates", () => {
  it("skips the source selector and shows built-in templates directly when there are no user templates", async () => {
    const session = makeSession({
      listUserDefTemplates: vi.fn().mockResolvedValue([]),
    });
    render(
      <CreateDefWizard
        catalog={makeCatalog()}
        session={session}
        onClose={vi.fn()}
        onCreated={vi.fn()}
      />,
    );

    await userEvent.click(screen.getByText("Thing"));

    await waitFor(() => {
      expect(session.listUserDefTemplates).toHaveBeenCalledWith("ThingDef");
    });

    expect(screen.queryByRole("tab", { name: "User Templates" })).toBeNull();
    expect(screen.getByText("Blank")).toBeTruthy();
    expect(screen.getByText("Weapon base")).toBeTruthy();
  });

  it("shows a source selector defaulting to user templates when they exist for the def type", async () => {
    const session = makeSession({
      listUserDefTemplates: vi.fn().mockResolvedValue([makeUserTemplate()]),
    });
    render(
      <CreateDefWizard
        catalog={makeCatalog()}
        session={session}
        onClose={vi.fn()}
        onCreated={vi.fn()}
      />,
    );

    await userEvent.click(screen.getByText("Thing"));

    expect(await screen.findByText("Autopistol base")).toBeTruthy();
    expect(screen.getByRole("tab", { name: "User Templates" })).toBeTruthy();
    expect(screen.getByRole("tab", { name: "Built-in Templates" })).toBeTruthy();
    // Built-in list is not shown while the user-templates tab is active.
    expect(screen.queryByText("Blank")).toBeNull();

    await userEvent.click(screen.getByRole("tab", { name: "Built-in Templates" }));
    expect(screen.getByText("Blank")).toBeTruthy();
    expect(screen.getByText("Weapon base")).toBeTruthy();
    expect(screen.queryByText("Autopistol base")).toBeNull();
  });

  it("does not show user templates saved for a different def type", async () => {
    const session = makeSession({
      listUserDefTemplates: vi.fn().mockImplementation((defType: string) =>
        Promise.resolve(defType === "ThingDef" ? [makeUserTemplate()] : []),
      ),
    });
    render(
      <CreateDefWizard
        catalog={makeCatalog()}
        session={session}
        onClose={vi.fn()}
        onCreated={vi.fn()}
      />,
    );

    await userEvent.click(screen.getByText("Pawn Kind"));

    await waitFor(() => {
      expect(session.listUserDefTemplates).toHaveBeenCalledWith("PawnKindDef");
    });
    expect(screen.queryByRole("tab", { name: "User Templates" })).toBeNull();
    expect(screen.queryByText("Autopistol base")).toBeNull();
  });

  it("clears the previous def type's user templates immediately on def-type switch, before the new fetch resolves", async () => {
    let resolveThingTemplates: (templates: UserDefTemplateSummary[]) => void =
      () => {};
    const thingTemplatesPromise = new Promise<UserDefTemplateSummary[]>(
      (resolve) => {
        resolveThingTemplates = resolve;
      },
    );
    const session = makeSession({
      listUserDefTemplates: vi.fn().mockImplementation((defType: string) =>
        defType === "ThingDef" ? thingTemplatesPromise : new Promise(() => {}),
      ),
    });
    render(
      <CreateDefWizard
        catalog={makeCatalog()}
        session={session}
        onClose={vi.fn()}
        onCreated={vi.fn()}
      />,
    );

    await userEvent.click(screen.getByText("Thing"));
    resolveThingTemplates([makeUserTemplate()]);
    expect(await screen.findByText("Autopistol base")).toBeTruthy();

    await userEvent.click(screen.getByText("Back"));
    await userEvent.click(screen.getByText("Pawn Kind"));

    // The PawnKindDef fetch is left unresolved (never() promise) - if the
    // previous ThingDef result weren't cleared synchronously, it would still
    // be showing here.
    expect(screen.queryByText("Autopistol base")).toBeNull();
    expect(screen.queryByRole("tab", { name: "User Templates" })).toBeNull();
  });

  it("prompts only for defName when creating from a user template, and requires it", async () => {
    const session = makeSession({
      listUserDefTemplates: vi.fn().mockResolvedValue([makeUserTemplate()]),
    });
    render(
      <CreateDefWizard
        catalog={makeCatalog()}
        session={session}
        onClose={vi.fn()}
        onCreated={vi.fn()}
      />,
    );

    await userEvent.click(screen.getByText("Thing"));
    await userEvent.click(await screen.findByText("Autopistol base"));

    expect(screen.getByText("Def Name")).toBeTruthy();
    expect(screen.queryByText("Label")).toBeNull();

    const createBtn = screen.getByText("Create") as HTMLButtonElement;
    expect(createBtn.disabled).toBe(true);

    const input = screen.getByPlaceholderText("e.g. MyThing");
    await userEvent.type(input, "Gun_MyPistol");
    expect(createBtn.disabled).toBe(false);

    await userEvent.click(createBtn);

    await waitFor(() => {
      expect(session.insertDefFromUserTemplate).toHaveBeenCalledWith(
        "tpl-1",
        "Gun_MyPistol",
      );
    });
  });

  it("deletes a user template after confirming, and refreshes the list", async () => {
    confirmMock.mockResolvedValue(true);
    const deleteUserDefTemplate = vi.fn().mockResolvedValue(undefined);
    const listUserDefTemplates = vi
      .fn()
      .mockResolvedValueOnce([
        makeUserTemplate(),
        makeUserTemplate({ id: "tpl-2", name: "Second template" }),
      ])
      .mockResolvedValueOnce([
        makeUserTemplate({ id: "tpl-2", name: "Second template" }),
      ]);
    const session = makeSession({ listUserDefTemplates, deleteUserDefTemplate });
    render(
      <CreateDefWizard
        catalog={makeCatalog()}
        session={session}
        onClose={vi.fn()}
        onCreated={vi.fn()}
      />,
    );

    await userEvent.click(screen.getByText("Thing"));
    await screen.findByText("Autopistol base");

    await userEvent.click(
      screen.getByRole("button", { name: "Delete template Autopistol base" }),
    );

    expect(confirmMock).toHaveBeenCalled();
    await waitFor(() => {
      expect(deleteUserDefTemplate).toHaveBeenCalledWith("tpl-1");
    });
    await waitFor(() => {
      expect(listUserDefTemplates).toHaveBeenCalledTimes(2);
    });
    expect(screen.queryByText("Autopistol base")).toBeNull();
    expect(screen.getByText("Second template")).toBeTruthy();
    // Other user templates remain, so the source tab stays on User Templates.
    expect(
      screen.getByRole("tab", { name: "User Templates" }).getAttribute(
        "aria-selected",
      ),
    ).toBe("true");
  });

  it("leaves the template unchanged when the delete confirmation is canceled", async () => {
    confirmMock.mockResolvedValue(false);
    const deleteUserDefTemplate = vi.fn().mockResolvedValue(undefined);
    const session = makeSession({
      listUserDefTemplates: vi.fn().mockResolvedValue([makeUserTemplate()]),
      deleteUserDefTemplate,
    });
    render(
      <CreateDefWizard
        catalog={makeCatalog()}
        session={session}
        onClose={vi.fn()}
        onCreated={vi.fn()}
      />,
    );

    await userEvent.click(screen.getByText("Thing"));
    await screen.findByText("Autopistol base");

    await userEvent.click(
      screen.getByRole("button", { name: "Delete template Autopistol base" }),
    );

    expect(confirmMock).toHaveBeenCalled();
    expect(deleteUserDefTemplate).not.toHaveBeenCalled();
    expect(screen.getByText("Autopistol base")).toBeTruthy();
  });

  it("switches back to built-in templates when the last user template is deleted", async () => {
    confirmMock.mockResolvedValue(true);
    const deleteUserDefTemplate = vi.fn().mockResolvedValue(undefined);
    const listUserDefTemplates = vi
      .fn()
      .mockResolvedValueOnce([makeUserTemplate()])
      .mockResolvedValueOnce([]);
    const session = makeSession({ listUserDefTemplates, deleteUserDefTemplate });
    render(
      <CreateDefWizard
        catalog={makeCatalog()}
        session={session}
        onClose={vi.fn()}
        onCreated={vi.fn()}
      />,
    );

    await userEvent.click(screen.getByText("Thing"));
    await screen.findByText("Autopistol base");

    await userEvent.click(
      screen.getByRole("button", { name: "Delete template Autopistol base" }),
    );

    await waitFor(() => {
      expect(deleteUserDefTemplate).toHaveBeenCalledWith("tpl-1");
    });

    // No user templates remain, so the tab UI disappears and the built-in list shows.
    await waitFor(() => {
      expect(screen.queryByRole("tab", { name: "User Templates" })).toBeNull();
    });
    expect(screen.getByText("Blank")).toBeTruthy();
    expect(screen.getByText("Weapon base")).toBeTruthy();
  });

  it("does not apply a stale delete refresh after the def type is switched mid-delete", async () => {
    confirmMock.mockResolvedValue(true);
    const deleteUserDefTemplate = vi.fn().mockResolvedValue(undefined);
    let resolveDeleteRefresh: (templates: UserDefTemplateSummary[]) => void =
      () => {};
    const deleteRefreshPromise = new Promise<UserDefTemplateSummary[]>(
      (resolve) => {
        resolveDeleteRefresh = resolve;
      },
    );
    let thingDefCalls = 0;
    const listUserDefTemplates = vi.fn().mockImplementation((defType: string) => {
      if (defType === "ThingDef") {
        thingDefCalls += 1;
        // 1st call: the def-type-select effect's initial load. 2nd call: the
        // delete handler's post-delete refresh, held open until resolved below.
        return thingDefCalls === 1
          ? Promise.resolve([makeUserTemplate()])
          : deleteRefreshPromise;
      }
      return Promise.resolve([]);
    });
    const session = makeSession({ listUserDefTemplates, deleteUserDefTemplate });
    render(
      <CreateDefWizard
        catalog={makeCatalog()}
        session={session}
        onClose={vi.fn()}
        onCreated={vi.fn()}
      />,
    );

    await userEvent.click(screen.getByText("Thing"));
    await screen.findByText("Autopistol base");

    await userEvent.click(
      screen.getByRole("button", { name: "Delete template Autopistol base" }),
    );
    await waitFor(() => {
      expect(deleteUserDefTemplate).toHaveBeenCalledWith("tpl-1");
    });
    // The refresh fetch for ThingDef is still pending here (deleteRefreshPromise).

    await userEvent.click(screen.getByText("Back"));
    await userEvent.click(screen.getByText("Pawn Kind"));
    expect(screen.queryByText("Autopistol base")).toBeNull();

    // Resolve the stale ThingDef refresh now that PawnKindDef is selected - it
    // must not repopulate the (now unrelated) user templates list or tab.
    await act(async () => {
      resolveDeleteRefresh([makeUserTemplate()]);
      await deleteRefreshPromise;
    });

    expect(screen.queryByText("Autopistol base")).toBeNull();
    expect(screen.queryByRole("tab", { name: "User Templates" })).toBeNull();
  });
});

describe("CreateDefWizard - built-in templates", () => {
  it("still supports the built-in create flow when no user templates exist", async () => {
    const session = makeSession();
    const onCreated = vi.fn();
    render(
      <CreateDefWizard
        catalog={makeCatalog()}
        session={session}
        onClose={vi.fn()}
        onCreated={onCreated}
      />,
    );

    await userEvent.click(screen.getByText("Thing"));
    await userEvent.click(await screen.findByText("Blank"));

    const input = screen.getByPlaceholderText("e.g. MyThing");
    await userEvent.type(input, "Gun_New");
    await userEvent.click(screen.getByText("Create"));

    await waitFor(() => {
      expect(session.insertDefFromTemplate).toHaveBeenCalledWith(
        "ThingDef",
        null,
        { defName: "Gun_New" },
      );
    });
    expect(onCreated).toHaveBeenCalled();
  });
});

describe("CreateDefWizard - indexed defs", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "search_defs") return [];
      return undefined;
    });
  });

  it("shows the Indexed Defs tab for a selected def type", async () => {
    const session = makeSession();
    render(
      <CreateDefWizard
        catalog={makeCatalog()}
        session={session}
        onClose={vi.fn()}
        onCreated={vi.fn()}
      />,
    );

    await userEvent.click(screen.getByText("Thing"));
    expect(screen.getByRole("tab", { name: "Indexed Defs" })).toBeTruthy();
  });

  it("searches with the selected def type and includeSources true", async () => {
    const session = makeSession();
    render(
      <CreateDefWizard
        catalog={makeCatalog()}
        session={session}
        onClose={vi.fn()}
        onCreated={vi.fn()}
      />,
    );

    await userEvent.click(screen.getByText("Thing"));
    await userEvent.click(screen.getByRole("tab", { name: "Indexed Defs" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("search_defs", {
        projectId: "proj1",
        query: "",
        defType: "ThingDef",
        includeSources: true,
        limit: 50,
      });
    });
  });

  it("clears stale indexed results when switching def type", async () => {
    invokeMock.mockImplementation(async (cmd: string, args?: unknown) => {
      const defType = (args as Record<string, unknown> | undefined)?.defType;
      if (cmd === "search_defs" && defType === "ThingDef") {
        return [makeIndexedSearchResult()];
      }
      return [];
    });
    const session = makeSession();
    render(
      <CreateDefWizard
        catalog={makeCatalog()}
        session={session}
        onClose={vi.fn()}
        onCreated={vi.fn()}
      />,
    );

    await userEvent.click(screen.getByText("Thing"));
    await userEvent.click(screen.getByRole("tab", { name: "Indexed Defs" }));
    await screen.findByText("autopistol");

    await userEvent.click(screen.getByText("Back"));
    await userEvent.click(screen.getByText("Pawn Kind"));
    await userEvent.click(screen.getByRole("tab", { name: "Indexed Defs" }));

    expect(screen.queryByText("autopistol")).toBeNull();
  });

  it("selecting an indexed def advances to step 3 and prompts only for defName", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "search_defs") return [makeIndexedSearchResult()];
      return [];
    });
    const session = makeSession();
    render(
      <CreateDefWizard
        catalog={makeCatalog()}
        session={session}
        onClose={vi.fn()}
        onCreated={vi.fn()}
      />,
    );

    await userEvent.click(screen.getByText("Thing"));
    await userEvent.click(screen.getByRole("tab", { name: "Indexed Defs" }));
    await userEvent.click(await screen.findByText("autopistol"));

    expect(screen.getByText("Def Name")).toBeTruthy();
    expect(screen.queryByText("Label")).toBeNull();
    const createBtn = screen.getByText("Create") as HTMLButtonElement;
    expect(createBtn.disabled).toBe(true);
  });

  it("creates from the selected indexed def with the entered defName", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "search_defs") return [makeIndexedSearchResult()];
      return [];
    });
    const session = makeSession();
    render(
      <CreateDefWizard
        catalog={makeCatalog()}
        session={session}
        onClose={vi.fn()}
        onCreated={vi.fn()}
      />,
    );

    await userEvent.click(screen.getByText("Thing"));
    await userEvent.click(screen.getByRole("tab", { name: "Indexed Defs" }));
    await userEvent.click(await screen.findByText("autopistol"));

    const input = screen.getByPlaceholderText("e.g. MyThing");
    await userEvent.type(input, "Gun_MyPistol");
    await userEvent.click(screen.getByText("Create"));

    await waitFor(() => {
      expect(session.insertDefFromIndexedDef).toHaveBeenCalledWith(
        makeIndexedSearchResult().def,
        "Gun_MyPistol",
      );
    });
  });
});

describe("CreateDefWizard - structured backend errors", () => {
  it("shows the backend message (not [object Object]) when insertDefFromIndexedDef rejects with a structured AppError", async () => {
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "search_defs") return [makeIndexedSearchResult()];
      return [];
    });
    const session = makeSession({
      insertDefFromIndexedDef: vi
        .fn()
        .mockRejectedValue({ code: "clone_failed", message: "Source def not found", details: null }),
    });
    render(
      <CreateDefWizard
        catalog={makeCatalog()}
        session={session}
        onClose={vi.fn()}
        onCreated={vi.fn()}
      />,
    );

    await userEvent.click(screen.getByText("Thing"));
    await userEvent.click(screen.getByRole("tab", { name: "Indexed Defs" }));
    await userEvent.click(await screen.findByText("autopistol"));

    const input = screen.getByPlaceholderText("e.g. MyThing");
    await userEvent.type(input, "Gun_MyPistol");
    await userEvent.click(screen.getByText("Create"));

    expect(await screen.findByText("Source def not found")).toBeTruthy();
    expect(screen.queryByText("[object Object]")).toBeNull();
  });

  it("shows the backend message when search_defs rejects with a structured AppError", async () => {
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "search_defs") {
        throw { code: "index_unavailable", message: "Def index is still building", details: null };
      }
      return [];
    });
    const session = makeSession();
    render(
      <CreateDefWizard
        catalog={makeCatalog()}
        session={session}
        onClose={vi.fn()}
        onCreated={vi.fn()}
      />,
    );

    await userEvent.click(screen.getByText("Thing"));
    await userEvent.click(screen.getByRole("tab", { name: "Indexed Defs" }));

    expect(await screen.findByText("Def index is still building")).toBeTruthy();
    expect(screen.queryByText("[object Object]")).toBeNull();
  });

  it("shows the backend message in the delete banner when deleteUserDefTemplate rejects with a structured AppError", async () => {
    confirmMock.mockResolvedValue(true);
    const session = makeSession({
      listUserDefTemplates: vi.fn().mockResolvedValue([makeUserTemplate()]),
      deleteUserDefTemplate: vi
        .fn()
        .mockRejectedValue({ code: "template_locked", message: "Template is in use", details: null }),
    });
    render(
      <CreateDefWizard
        catalog={makeCatalog()}
        session={session}
        onClose={vi.fn()}
        onCreated={vi.fn()}
      />,
    );

    await userEvent.click(screen.getByText("Thing"));
    await screen.findByText("Autopistol base");

    await userEvent.click(
      screen.getByRole("button", { name: "Delete template Autopistol base" }),
    );

    expect(await screen.findByText("Template is in use")).toBeTruthy();
    expect(screen.queryByText("[object Object]")).toBeNull();
  });
});
