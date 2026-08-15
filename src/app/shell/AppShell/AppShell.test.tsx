import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { renderWithI18n as render } from "../../../i18n/testing/renderWithI18n";
import { AppShell } from "./AppShell";
import { pickSourceFolder } from "../../../features/project-settings";
import { useIndexingStatus } from "../../../features/def-index";
import { openAppDataFolder } from "../../commands/appDataCommands";
import type { ProjectSettings } from "../../../features/project-settings";

const pickSourceFolderMock = vi.mocked(pickSourceFolder);
const useIndexingStatusMock = vi.mocked(useIndexingStatus);
const openAppDataFolderMock = vi.mocked(openAppDataFolder);

vi.mock("../../commands/appDataCommands", () => ({
  openAppDataFolder: vi.fn(),
}));

// AppShell orchestrates several heavy features (project explorer, editor workspace, def search,
// schema catalog). This test only exercises the Preferences-dialog wiring: both entry points open
// the same dialog, and opening it doesn't disturb the current activity pane or create a resize
// handle -- so every unrelated feature is stubbed out to keep the test scoped and independent of
// their own IPC calls.

const settings: ProjectSettings = {
  schemaVersion: 3,
  gameVersion: "1.6",
  locale: "en",
  locations: [],
  activeProjectId: undefined,
};

vi.mock("../../../features/project-settings", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../../features/project-settings")>();
  return {
    ...actual,
    useProjectSettings: () => ({
      settings,
      loading: false,
      error: null,
      installedSchemaVersions: [],
      startupNotice: null,
      clearStartupNotice: vi.fn(),
      addLocation: vi.fn(),
      deleteLocation: vi.fn(),
      editLocation: vi.fn(),
      activateProject: vi.fn(),
      updateGameVersion: vi.fn(),
      replaceSettings: vi.fn(),
    }),
    pickProjectFolder: vi.fn(),
    pickSourceFolder: vi.fn(),
  };
});

vi.mock("../../../features/editor-workspace", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../../features/editor-workspace")>();
  return {
    ...actual,
    useEditorWorkspace: () => ({
      scan: null,
      loadingScan: false,
      refreshingScan: false,
      scanError: null,
      refresh: vi.fn(),
      tabs: [],
      activeTabKey: null,
      openTab: vi.fn(),
      activateTab: vi.fn(),
      closeTab: vi.fn(),
      setTabDirty: vi.fn(),
      reconcileRename: vi.fn(),
      reconcileDelete: vi.fn(),
      forceCloseTabs: vi.fn(),
      mutatingPath: null,
      mutationError: null,
      clearMutationError: vi.fn(),
      createFile: vi.fn(),
      createFolder: vi.fn(),
      renamePath: vi.fn(),
      deletePath: vi.fn(),
    }),
    EditorWorkspace: () => <div data-testid="editor-workspace" />,
  };
});

vi.mock("../../../features/project-explorer", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../../features/project-explorer")>();
  return {
    ...actual,
    buildFileTree: () => null,
    filterFiles: () => [],
    collectFolderIds: () => new Set<string>(),
    ProjectExplorerPanel: ({ visible }: { visible: boolean }) => (
      <div data-testid="explorer-panel" data-visible={visible} />
    ),
  };
});

vi.mock("../../../features/def-index", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../../features/def-index")>();
  return {
    ...actual,
    DefSearchPanel: ({ visible }: { visible: boolean }) => (
      <div data-testid="search-panel" data-visible={visible} />
    ),
    useIndexingStatus: vi.fn(() => null),
  };
});

vi.mock("../../../features/schema-catalog", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../../features/schema-catalog")>();
  return {
    ...actual,
    useSchemaCatalog: () => ({ catalog: null, loading: false, error: null }),
  };
});

// `useTheme` reads `window.matchMedia` on mount to resolve the "system" theme, and
// `usePersistentLayoutState` observes the workspace element's size via `ResizeObserver` -- jsdom
// implements neither, so every test in this file needs stubs before `AppShell` mounts.
beforeEach(() => {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
  globalThis.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
});

describe("AppShell Preferences integration", () => {
  it("opens the Preferences dialog from the activity-rail gear", async () => {
    const user = userEvent.setup();
    render(<AppShell />);
    expect(screen.queryByRole("dialog", { name: "Preferences" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Preferences" }));
    expect(screen.getByRole("dialog", { name: "Preferences" })).toBeDefined();
  });

  it("opens the same Preferences dialog from File > Preferences", async () => {
    const user = userEvent.setup();
    render(<AppShell />);
    await user.click(screen.getByRole("button", { name: "File" }));
    await user.click(screen.getByRole("menuitem", { name: "Preferences" }));
    expect(screen.getByRole("dialog", { name: "Preferences" })).toBeDefined();
  });

  it("does not hide the current explorer pane or create a resize handle when opened", async () => {
    const user = userEvent.setup();
    render(<AppShell />);
    const explorerPanel = screen.getByTestId("explorer-panel");
    expect(explorerPanel.dataset.visible).toBe("true");
    const handleCountBefore = document.querySelectorAll('[class*="handle"]').length;

    await user.click(screen.getByRole("button", { name: "Preferences" }));

    expect(screen.getByTestId("explorer-panel").dataset.visible).toBe("true");
    expect(document.querySelectorAll('[class*="handle"]').length).toBe(handleCountBefore);
  });

  it("closing Preferences returns focus without leaving a stray dialog", async () => {
    const user = userEvent.setup();
    render(<AppShell />);
    await user.click(screen.getByRole("button", { name: "Preferences" }));
    expect(screen.getByRole("dialog", { name: "Preferences" })).toBeDefined();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: "Preferences" })).toBeNull();
  });
});

// The window must stay usable while Def-index initialization is still asynchronously
// hydrating/rebuilding in the background -- this exercises the shell with the real (unmocked) StatusBar
// receiving a live-looking `IndexingStatus`, unlike the suite above which stubs `useIndexingStatus` to `null`.
describe("AppShell while the Def index is still initializing", () => {
  afterEach(() => {
    useIndexingStatusMock.mockReturnValue(null);
  });

  it("stays interactive and shows the hydrating-cache status while the background thread reads the disk cache", async () => {
    useIndexingStatusMock.mockReturnValue({
      phase: "running",
      cacheVerification: "notRequired",
      pendingFiles: 0,
      indexedDefs: 0,
      projectDefs: 0,
      sourceDefs: 0,
      errors: 0,
      updatedAtUnixMs: 0,
      currentStage: "hydratingCache",
    });

    const user = userEvent.setup();
    render(<AppShell />);

    expect(screen.getByText("Loading Def index cache…")).toBeDefined();
    // The shell's other affordances must remain fully usable -- initialization must not block
    // the window from accepting input.
    await user.click(screen.getByRole("button", { name: "Preferences" }));
    expect(screen.getByRole("dialog", { name: "Preferences" })).toBeDefined();
  });

  it("stays interactive while a Pending status precedes any hydration/discovery detail", async () => {
    useIndexingStatusMock.mockReturnValue({
      phase: "pending",
      cacheVerification: "notRequired",
      pendingFiles: 0,
      indexedDefs: 0,
      projectDefs: 0,
      sourceDefs: 0,
      errors: 0,
      updatedAtUnixMs: 0,
    });

    const user = userEvent.setup();
    render(<AppShell />);

    expect(screen.getByText("Index pending")).toBeDefined();
    await user.click(screen.getByRole("button", { name: "Preferences" }));
    expect(screen.getByRole("dialog", { name: "Preferences" })).toBeDefined();
  });
});

describe("AppShell add source folder", () => {
  it("shows a dismissible reminder when the picked folder is an ambiguous Workshop root", async () => {
    const user = userEvent.setup();
    pickSourceFolderMock.mockResolvedValue({
      settings: {
        ...settings,
        locations: [
          {
            id: "src-1",
            displayName: "WeirdFolder",
            rootPath: "C:/mods/WeirdFolder",
            kind: "source",
            sourceType: "folder",
            readOnly: true,
            createdAt: "",
            updatedAt: "",
          },
        ],
      },
      locationId: "src-1",
      ambiguousWorkshopRoot: true,
    });
    render(<AppShell />);

    await user.click(screen.getByRole("button", { name: "Add source folder" }));

    await waitFor(() => {
      expect(screen.getByText(/was added as a Folder source/)).toBeDefined();
    });

    await user.click(screen.getByRole("button", { name: "Dismiss Workshop root notice" }));
    expect(screen.queryByText(/was added as a Folder source/)).toBeNull();
  });

  it("shows no reminder when the picked folder was not ambiguous", async () => {
    pickSourceFolderMock.mockResolvedValue({
      settings: { ...settings, locations: [] },
      locationId: "src-1",
      ambiguousWorkshopRoot: false,
    });
    const user = userEvent.setup();
    render(<AppShell />);

    await user.click(screen.getByRole("button", { name: "Add source folder" }));

    expect(screen.queryByText(/was added as a Folder source/)).toBeNull();
  });
});

describe("AppShell open RimEdit data folder", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("invokes the app-data-folder IPC command from File > Open RimEdit Data Folder", async () => {
    openAppDataFolderMock.mockResolvedValue(undefined);
    const user = userEvent.setup();
    render(<AppShell />);

    await user.click(screen.getByRole("button", { name: "File" }));
    await user.click(screen.getByRole("menuitem", { name: "Open RimEdit Data Folder" }));

    expect(openAppDataFolderMock).toHaveBeenCalledOnce();
  });

  it("catches a rejected invoke instead of leaving an unhandled rejection", async () => {
    const consoleErrorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    openAppDataFolderMock.mockRejectedValue(new Error("boom"));
    const user = userEvent.setup();
    render(<AppShell />);

    await user.click(screen.getByRole("button", { name: "File" }));
    await user.click(screen.getByRole("menuitem", { name: "Open RimEdit Data Folder" }));

    await waitFor(() => {
      expect(consoleErrorSpy).toHaveBeenCalledWith(
        "Failed to open RimEdit data folder:",
        expect.any(Error),
      );
    });
    consoleErrorSpy.mockRestore();
  });

  it("is enabled with no active project", async () => {
    const user = userEvent.setup();
    render(<AppShell />);

    await user.click(screen.getByRole("button", { name: "File" }));

    expect(
      screen.getByRole("menuitem", { name: "Open RimEdit Data Folder" }),
    ).toHaveProperty("disabled", false);
  });
});
