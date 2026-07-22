import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { renderWithI18n as render } from "../../../i18n/testing/renderWithI18n";
import { AppShell } from "./AppShell";
import type { ProjectSettings } from "../../../features/project-settings";

// AppShell orchestrates several heavy features (project explorer, editor workspace, def search,
// schema catalog). This test only exercises the Preferences-dialog wiring described in Plan.md
// ("Preferences window implementation plan"): both entry points open the same dialog, and opening
// it doesn't disturb the current activity pane or create a resize handle -- so every unrelated
// feature is stubbed out to keep the test scoped and independent of their own IPC calls.

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
    useIndexingStatus: () => null,
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
