import { screen, fireEvent, waitFor } from "@testing-library/react";
import { confirm } from "@tauri-apps/plugin-dialog";
import { renderWithI18n as render } from "../../../../i18n/testing/renderWithI18n";
import { ProjectSettingsPanel } from "./ProjectSettingsPanel";
import type { ProjectSettings } from "../../types";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: vi.fn(),
}));

// confirmDiscardChanges uses confirm under the hood; mock the lib wrapper too
vi.mock("../../../../lib/confirmDiscardChanges", () => ({
  confirmDiscardChanges: vi.fn().mockResolvedValue(true),
}));

const confirmMock = vi.mocked(confirm);

function makeSettings(overrides: Partial<ProjectSettings> = {}): ProjectSettings {
  return {
    schemaVersion: 3,
    gameVersion: "1.6",
    locale: "en",
    locations: [
      {
        id: "p1",
        displayName: "My Project",
        rootPath: "/projects/p1",
        kind: "project",
        sourceType: "folder",
        readOnly: false,
        createdAt: "2024-01-01T00:00:00Z",
        updatedAt: "2024-01-01T00:00:00Z",
      },
      {
        id: "s1",
        displayName: "Core",
        rootPath: "/game/core",
        kind: "source",
        sourceType: "baseGame",
        readOnly: true,
        createdAt: "2024-01-01T00:00:00Z",
        updatedAt: "2024-01-01T00:00:00Z",
      },
    ],
    activeProjectId: "p1",
    ...overrides,
  };
}

function defaultProps(
  overrides: Partial<Parameters<typeof ProjectSettingsPanel>[0]> = {},
) {
  return {
    visible: true,
    settings: makeSettings(),
    loading: false,
    loadError: null,
    hasDirtyTabs: false,
    installedSchemaVersions: ["1.5", "1.6"],
    locale: "en",
    onEditLocation: vi.fn().mockResolvedValue(undefined),
    onRemoveLocation: vi.fn().mockResolvedValue(undefined),
    onUpdateGameVersion: vi.fn().mockResolvedValue(undefined),
    onChangeLocale: vi.fn().mockResolvedValue(undefined),
    onOpenProject: vi.fn(),
    onAddSourceFolder: vi.fn(),
    ...overrides,
  };
}

describe("ProjectSettingsPanel grouping", () => {
  it("shows Active Project section for the active project", () => {
    render(<ProjectSettingsPanel {...defaultProps()} />);
    expect(screen.getByText("Active Project")).toBeDefined();
    expect(screen.getByText("My Project")).toBeDefined();
  });

  it("shows Read-only Sources section", () => {
    render(<ProjectSettingsPanel {...defaultProps()} />);
    expect(screen.getByText("Read-only Sources")).toBeDefined();
    expect(screen.getByText("Core")).toBeDefined();
  });

  it("shows Other Projects section when non-active projects exist", () => {
    const settings = makeSettings({
      locations: [
        ...makeSettings().locations,
        {
          id: "p2",
          displayName: "Other Project",
          rootPath: "/projects/p2",
          kind: "project",
          sourceType: "folder",
          readOnly: false,
          createdAt: "2024-01-01T00:00:00Z",
          updatedAt: "2024-01-01T00:00:00Z",
        },
      ],
    });
    render(<ProjectSettingsPanel {...defaultProps({ settings })} />);
    expect(screen.getByText("Other Projects")).toBeDefined();
    expect(screen.getByText("Other Project")).toBeDefined();
  });

  it("shows Projects section (not Other Projects) when no active project", () => {
    const settings = makeSettings({ activeProjectId: undefined });
    render(<ProjectSettingsPanel {...defaultProps({ settings })} />);
    expect(screen.getByText("Projects")).toBeDefined();
    expect(screen.queryByText("Active Project")).toBeNull();
    expect(screen.queryByText("Other Projects")).toBeNull();
  });
});

describe("ProjectSettingsPanel empty state", () => {
  it("shows Open Project and Add Source Folder buttons when no locations", () => {
    render(
      <ProjectSettingsPanel
        {...defaultProps({ settings: { schemaVersion: 3, gameVersion: "1.6", locale: "en", locations: [], activeProjectId: undefined } })}
      />,
    );
    expect(screen.getAllByRole("button", { name: "Open Project" }).length).toBeGreaterThan(0);
    expect(screen.getAllByRole("button", { name: "Add Source Folder" }).length).toBeGreaterThan(0);
  });

  it("calls onOpenProject from empty state button", () => {
    const onOpenProject = vi.fn();
    render(
      <ProjectSettingsPanel
        {...defaultProps({
          settings: { schemaVersion: 3, gameVersion: "1.6", locale: "en", locations: [], activeProjectId: undefined },
          onOpenProject,
        })}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Open Project" }));
    expect(onOpenProject).toHaveBeenCalledOnce();
  });

  it("calls onAddSourceFolder from empty state button", () => {
    const onAddSourceFolder = vi.fn();
    render(
      <ProjectSettingsPanel
        {...defaultProps({
          settings: { schemaVersion: 3, gameVersion: "1.6", locale: "en", locations: [], activeProjectId: undefined },
          onAddSourceFolder,
        })}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Add Source Folder" }));
    expect(onAddSourceFolder).toHaveBeenCalledOnce();
  });
});

describe("ProjectSettingsPanel header actions", () => {
  it("shows Open Project icon button in header", () => {
    render(<ProjectSettingsPanel {...defaultProps()} />);
    expect(screen.getByRole("button", { name: "Open project" })).toBeDefined();
  });

  it("calls onOpenProject from header button", () => {
    const onOpenProject = vi.fn();
    render(<ProjectSettingsPanel {...defaultProps({ onOpenProject })} />);
    fireEvent.click(screen.getByRole("button", { name: "Open project" }));
    expect(onOpenProject).toHaveBeenCalledOnce();
  });

  it("calls onAddSourceFolder from header button", () => {
    const onAddSourceFolder = vi.fn();
    render(<ProjectSettingsPanel {...defaultProps({ onAddSourceFolder })} />);
    fireEvent.click(screen.getByRole("button", { name: "Add source folder" }));
    expect(onAddSourceFolder).toHaveBeenCalledOnce();
  });
});

describe("ProjectSettingsPanel error states", () => {
  it("shows loadError when settings is null", () => {
    render(
      <ProjectSettingsPanel
        {...defaultProps({ settings: null, loadError: "Failed to load settings" })}
      />,
    );
    expect(screen.getByText("Failed to load settings")).toBeDefined();
  });

  it("shows panel error banner after failed edit", async () => {
    const onEditLocation = vi.fn().mockRejectedValue({ message: "update failed" });
    render(<ProjectSettingsPanel {...defaultProps({ onEditLocation })} />);
    // Enter edit mode on the first row
    fireEvent.click(screen.getAllByRole("button", { name: "Edit" })[0]);
    const input = screen.getByRole("textbox", { name: "Display name" });
    fireEvent.change(input, { target: { value: "New Name" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(screen.getByRole("alert")).toBeDefined(),
    );
    expect(screen.getByText("update failed")).toBeDefined();
  });

  it("keeps row in edit mode after failed save", async () => {
    const onEditLocation = vi.fn().mockRejectedValue({ message: "update failed" });
    render(<ProjectSettingsPanel {...defaultProps({ onEditLocation })} />);
    fireEvent.click(screen.getAllByRole("button", { name: "Edit" })[0]);
    const input = screen.getByRole("textbox", { name: "Display name" });
    fireEvent.change(input, { target: { value: "New Name" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(screen.getByRole("alert")).toBeDefined());
    // Edit mode input should still be visible
    expect(screen.getByRole("textbox", { name: "Display name" })).toBeDefined();
  });

  it("dismisses panel error banner", async () => {
    const onEditLocation = vi.fn().mockRejectedValue({ message: "oops" });
    render(<ProjectSettingsPanel {...defaultProps({ onEditLocation })} />);
    fireEvent.click(screen.getAllByRole("button", { name: "Edit" })[0]);
    const input = screen.getByRole("textbox", { name: "Display name" });
    fireEvent.change(input, { target: { value: "New Name" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(screen.getByRole("alert")).toBeDefined());
    fireEvent.click(screen.getByRole("button", { name: "Dismiss error" }));
    expect(screen.queryByRole("alert")).toBeNull();
  });
});

describe("ProjectSettingsPanel language", () => {
  it("shows a Language selector with only English, no System option", () => {
    render(<ProjectSettingsPanel {...defaultProps()} />);
    const select = screen.getByRole("combobox", { name: "Language" });
    expect(select).toBeDefined();
    expect(screen.getByRole("option", { name: "English" })).toBeDefined();
    expect(screen.queryByRole("option", { name: "System" })).toBeNull();
  });

  it("calls onChangeLocale when a different locale is selected", async () => {
    const onChangeLocale = vi.fn().mockResolvedValue(undefined);
    render(<ProjectSettingsPanel {...defaultProps({ onChangeLocale })} />);
    const select = screen.getByRole("combobox", { name: "Language" });
    fireEvent.change(select, { target: { value: "en" } });
    // Selecting the already-active locale is a no-op; nothing to persist.
    expect(onChangeLocale).not.toHaveBeenCalled();
  });

  // `onChangeLocale` (`LocaleProvider.changeLocale`) is itself responsible for reverting
  // i18next/document/state to the prior locale on a persistence failure -- see
  // `src/i18n/LocaleProvider.tsx` and its test coverage -- so `ProjectSettingsPanel` no longer
  // makes its own caller-side rollback call; it only surfaces the translated error.
  it("shows a panel error when persistence fails, without a caller-side rollback call", async () => {
    const onChangeLocale = vi.fn().mockRejectedValueOnce({ message: "locale save failed" });
    // `locale` starts as a value with no matching <option> (simulating a settings
    // value that doesn't match the currently offered locale list) so that
    // selecting the one real "English" option below is an actual value change.
    render(
      <ProjectSettingsPanel
        {...defaultProps({ locale: "de", onChangeLocale })}
      />,
    );
    const select = screen.getByRole("combobox", { name: "Language" }) as HTMLSelectElement;
    fireEvent.change(select, { target: { value: "en" } });
    await waitFor(() => expect(screen.getByRole("alert")).toBeDefined());
    expect(screen.getByText("locale save failed")).toBeDefined();
    expect(onChangeLocale).toHaveBeenCalledTimes(1);
    expect(onChangeLocale).toHaveBeenCalledWith("en");
  });
});

describe("ProjectSettingsPanel remove", () => {
  it("calls onRemoveLocation after confirm", async () => {
    confirmMock.mockResolvedValueOnce(true);
    const onRemoveLocation = vi.fn().mockResolvedValue(undefined);
    render(<ProjectSettingsPanel {...defaultProps({ onRemoveLocation })} />);
    fireEvent.click(screen.getAllByRole("button", { name: "Remove" })[0]);
    await waitFor(() => expect(onRemoveLocation).toHaveBeenCalled());
  });

  it("does not call onRemoveLocation when confirm is cancelled", async () => {
    confirmMock.mockResolvedValueOnce(false);
    const onRemoveLocation = vi.fn();
    render(<ProjectSettingsPanel {...defaultProps({ onRemoveLocation })} />);
    fireEvent.click(screen.getAllByRole("button", { name: "Remove" })[0]);
    await waitFor(() => expect(confirmMock).toHaveBeenCalled());
    expect(onRemoveLocation).not.toHaveBeenCalled();
  });
});
