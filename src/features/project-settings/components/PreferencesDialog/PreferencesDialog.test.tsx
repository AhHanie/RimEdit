import { screen, fireEvent, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { confirm } from "@tauri-apps/plugin-dialog";
import { renderWithI18n as render } from "../../../../i18n/testing/renderWithI18n";
import { confirmDiscardChanges } from "../../../../lib/confirmDiscardChanges";
import { PreferencesDialog } from "./PreferencesDialog";
import type { ProjectSettings } from "../../types";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: vi.fn(),
}));

// confirmDiscardChanges uses confirm under the hood; mock the lib wrapper too
vi.mock("../../../../lib/confirmDiscardChanges", () => ({
  confirmDiscardChanges: vi.fn().mockResolvedValue(true),
}));

const confirmMock = vi.mocked(confirm);
const confirmDiscardChangesMock = vi.mocked(confirmDiscardChanges);

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
  overrides: Partial<Parameters<typeof PreferencesDialog>[0]> = {},
) {
  return {
    onClose: vi.fn(),
    settings: makeSettings(),
    loading: false,
    loadError: null,
    hasDirtyTabs: false,
    installedSchemaVersions: ["1.5", "1.6"],
    locale: "en",
    themeMode: "system" as const,
    onChangeTheme: vi.fn(),
    onEditLocation: vi.fn().mockResolvedValue(undefined),
    onRemoveLocation: vi.fn().mockResolvedValue(undefined),
    onUpdateGameVersion: vi.fn().mockResolvedValue(undefined),
    onChangeLocale: vi.fn().mockResolvedValue(undefined),
    onOpenProject: vi.fn(),
    onAddSourceFolder: vi.fn(),
    ...overrides,
  };
}

function switchToCategory(name: string) {
  fireEvent.click(screen.getByRole("tab", { name }));
}

describe("PreferencesDialog structure", () => {
  it("opens on the General category by default", () => {
    render(<PreferencesDialog {...defaultProps()} />);
    expect(screen.getByRole("tab", { name: "General" }).getAttribute("aria-selected")).toBe(
      "true",
    );
    expect(screen.getByRole("radiogroup", { name: "Appearance" })).toBeDefined();
  });

  it("has three tabs with correct tab/tabpanel accessibility wiring", () => {
    render(<PreferencesDialog {...defaultProps()} />);
    const tabs = screen.getAllByRole("tab");
    expect(tabs.map((t) => t.textContent)).toEqual(["General", "RimWorld", "Locations"]);
    const activeTab = screen.getByRole("tab", { name: "General" });
    const panel = screen.getByRole("tabpanel");
    expect(activeTab.getAttribute("aria-controls")).toBe(panel.id);
    expect(panel.getAttribute("aria-labelledby")).toBe(activeTab.id);
  });

  it("switches categories on click", () => {
    render(<PreferencesDialog {...defaultProps()} />);
    switchToCategory("RimWorld");
    expect(screen.getByRole("tab", { name: "RimWorld" }).getAttribute("aria-selected")).toBe(
      "true",
    );
    expect(screen.getByRole("combobox", { name: "Game version" })).toBeDefined();
  });
});

describe("PreferencesDialog appearance", () => {
  it("marks the current theme mode as checked", () => {
    render(<PreferencesDialog {...defaultProps({ themeMode: "dark" })} />);
    const darkRadio = screen.getByRole("radio", { name: "Dark" }) as HTMLInputElement;
    expect(darkRadio.checked).toBe(true);
  });

  it("calls onChangeTheme when a different mode is selected", () => {
    const onChangeTheme = vi.fn();
    render(<PreferencesDialog {...defaultProps({ themeMode: "light", onChangeTheme })} />);
    fireEvent.click(screen.getByRole("radio", { name: "Dark" }));
    expect(onChangeTheme).toHaveBeenCalledWith("dark");
  });
});

describe("PreferencesDialog locations grouping", () => {
  it("explains what a location is", () => {
    render(<PreferencesDialog {...defaultProps()} />);
    switchToCategory("Locations");
    expect(
      screen.getByText(/A location is either your active project/),
    ).toBeDefined();
  });

  it("shows Active Project section for the active project", () => {
    render(<PreferencesDialog {...defaultProps()} />);
    switchToCategory("Locations");
    expect(screen.getByText("Active Project")).toBeDefined();
    expect(screen.getByText("My Project")).toBeDefined();
  });

  it("shows Read-only Sources section", () => {
    render(<PreferencesDialog {...defaultProps()} />);
    switchToCategory("Locations");
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
    render(<PreferencesDialog {...defaultProps({ settings })} />);
    switchToCategory("Locations");
    expect(screen.getByText("Other Projects")).toBeDefined();
    expect(screen.getByText("Other Project")).toBeDefined();
  });

  it("shows Projects section (not Other Projects) when no active project", () => {
    const settings = makeSettings({ activeProjectId: undefined });
    render(<PreferencesDialog {...defaultProps({ settings })} />);
    switchToCategory("Locations");
    expect(screen.getByText("Projects")).toBeDefined();
    expect(screen.queryByText("Active Project")).toBeNull();
    expect(screen.queryByText("Other Projects")).toBeNull();
  });
});

describe("PreferencesDialog locations empty state", () => {
  function emptySettings(): ProjectSettings {
    return { schemaVersion: 3, gameVersion: "1.6", locale: "en", locations: [], activeProjectId: undefined };
  }

  it("shows Open Project and Add Source Folder buttons when no locations", () => {
    render(<PreferencesDialog {...defaultProps({ settings: emptySettings() })} />);
    switchToCategory("Locations");
    expect(screen.getAllByRole("button", { name: "Open Project" }).length).toBeGreaterThan(0);
    expect(screen.getAllByRole("button", { name: "Add Source Folder" }).length).toBeGreaterThan(0);
  });

  it("calls onOpenProject from empty state button", () => {
    const onOpenProject = vi.fn();
    render(<PreferencesDialog {...defaultProps({ settings: emptySettings(), onOpenProject })} />);
    switchToCategory("Locations");
    fireEvent.click(screen.getByRole("button", { name: "Open Project" }));
    expect(onOpenProject).toHaveBeenCalledOnce();
  });

  it("calls onAddSourceFolder from empty state button", () => {
    const onAddSourceFolder = vi.fn();
    render(
      <PreferencesDialog {...defaultProps({ settings: emptySettings(), onAddSourceFolder })} />,
    );
    switchToCategory("Locations");
    fireEvent.click(screen.getByRole("button", { name: "Add Source Folder" }));
    expect(onAddSourceFolder).toHaveBeenCalledOnce();
  });
});

describe("PreferencesDialog locations header actions", () => {
  it("shows Open Project icon button in the Locations header", () => {
    render(<PreferencesDialog {...defaultProps()} />);
    switchToCategory("Locations");
    expect(screen.getByRole("button", { name: "Open project" })).toBeDefined();
  });

  it("calls onOpenProject from header button", () => {
    const onOpenProject = vi.fn();
    render(<PreferencesDialog {...defaultProps({ onOpenProject })} />);
    switchToCategory("Locations");
    fireEvent.click(screen.getByRole("button", { name: "Open project" }));
    expect(onOpenProject).toHaveBeenCalledOnce();
  });

  it("calls onAddSourceFolder from header button", () => {
    const onAddSourceFolder = vi.fn();
    render(<PreferencesDialog {...defaultProps({ onAddSourceFolder })} />);
    switchToCategory("Locations");
    fireEvent.click(screen.getByRole("button", { name: "Add source folder" }));
    expect(onAddSourceFolder).toHaveBeenCalledOnce();
  });
});

describe("PreferencesDialog error states", () => {
  it("shows loadError when settings is null", () => {
    render(
      <PreferencesDialog
        {...defaultProps({ settings: null, loadError: "Failed to load settings" })}
      />,
    );
    switchToCategory("Locations");
    expect(screen.getByText("Failed to load settings")).toBeDefined();
  });

  it("shows a dialog error banner after failed location edit and keeps the row in edit mode", async () => {
    const onEditLocation = vi.fn().mockRejectedValue({ message: "update failed" });
    render(<PreferencesDialog {...defaultProps({ onEditLocation })} />);
    switchToCategory("Locations");
    fireEvent.click(screen.getAllByRole("button", { name: "Edit" })[0]);
    const input = screen.getByRole("textbox", { name: "Display name" });
    fireEvent.change(input, { target: { value: "New Name" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(screen.getByRole("alert")).toBeDefined());
    expect(screen.getByText("update failed")).toBeDefined();
    expect(screen.getByRole("textbox", { name: "Display name" })).toBeDefined();
  });

  it("dismisses the error banner", async () => {
    const onEditLocation = vi.fn().mockRejectedValue({ message: "oops" });
    render(<PreferencesDialog {...defaultProps({ onEditLocation })} />);
    switchToCategory("Locations");
    fireEvent.click(screen.getAllByRole("button", { name: "Edit" })[0]);
    const input = screen.getByRole("textbox", { name: "Display name" });
    fireEvent.change(input, { target: { value: "New Name" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(screen.getByRole("alert")).toBeDefined());
    fireEvent.click(screen.getByRole("button", { name: "Dismiss error" }));
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("clears a category's error banner when switching to a different category", async () => {
    const onChangeLocale = vi.fn().mockRejectedValueOnce({ message: "locale save failed" });
    render(<PreferencesDialog {...defaultProps({ locale: "de", onChangeLocale })} />);
    const select = screen.getByRole("combobox", { name: "Language" });
    fireEvent.change(select, { target: { value: "en" } });
    await waitFor(() => expect(screen.getByRole("alert")).toBeDefined());

    switchToCategory("RimWorld");
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.queryByText("locale save failed")).toBeNull();
  });
});

describe("PreferencesDialog language", () => {
  it("shows a Language selector with only English, no System option", () => {
    render(<PreferencesDialog {...defaultProps()} />);
    const select = screen.getByRole("combobox", { name: "Language" });
    expect(select).toBeDefined();
    expect(screen.getByRole("option", { name: "English" })).toBeDefined();
    expect(screen.queryByRole("option", { name: "System" })).toBeNull();
  });

  it("does not call onChangeLocale when the already-active locale is re-selected", async () => {
    const onChangeLocale = vi.fn().mockResolvedValue(undefined);
    render(<PreferencesDialog {...defaultProps({ onChangeLocale })} />);
    const select = screen.getByRole("combobox", { name: "Language" });
    fireEvent.change(select, { target: { value: "en" } });
    // Selecting the already-active locale is a no-op; nothing to persist.
    expect(onChangeLocale).not.toHaveBeenCalled();
  });

  // `SUPPORTED_LOCALES` only ships "en" today, so there's no second real option to pick from the
  // dropdown -- these tests instead start from a `locale` prop value ("de") that has no matching
  // <option>, the same way the failure test below simulates an actual change to "en".
  it("calls onChangeLocale with the newly selected locale and clears any pending/error state on success", async () => {
    const onChangeLocale = vi.fn().mockResolvedValue(undefined);
    render(<PreferencesDialog {...defaultProps({ locale: "de", onChangeLocale })} />);
    const select = screen.getByRole("combobox", { name: "Language" }) as HTMLSelectElement;
    fireEvent.change(select, { target: { value: "en" } });
    await waitFor(() => expect(onChangeLocale).toHaveBeenCalledWith("en"));
    expect(onChangeLocale).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("alert")).toBeNull();
    expect(select.disabled).toBe(false);
  });

  it("disables the language selector while a locale change is pending", async () => {
    let resolveChange!: () => void;
    const onChangeLocale = vi.fn(
      () => new Promise<void>((resolve) => { resolveChange = resolve; }),
    );
    render(<PreferencesDialog {...defaultProps({ locale: "de", onChangeLocale })} />);
    const select = screen.getByRole("combobox", { name: "Language" }) as HTMLSelectElement;
    fireEvent.change(select, { target: { value: "en" } });
    await waitFor(() => expect(select.disabled).toBe(true));
    resolveChange();
    await waitFor(() => expect(select.disabled).toBe(false));
  });

  it("shows a dialog error when persistence fails, without a caller-side rollback call", async () => {
    const onChangeLocale = vi.fn().mockRejectedValueOnce({ message: "locale save failed" });
    render(<PreferencesDialog {...defaultProps({ locale: "de", onChangeLocale })} />);
    const select = screen.getByRole("combobox", { name: "Language" }) as HTMLSelectElement;
    fireEvent.change(select, { target: { value: "en" } });
    await waitFor(() => expect(screen.getByRole("alert")).toBeDefined());
    expect(screen.getByText("locale save failed")).toBeDefined();
    expect(onChangeLocale).toHaveBeenCalledTimes(1);
    expect(onChangeLocale).toHaveBeenCalledWith("en");
  });
});

describe("PreferencesDialog game version", () => {
  it("calls onUpdateGameVersion when a different version is selected", async () => {
    const onUpdateGameVersion = vi.fn().mockResolvedValue(undefined);
    render(<PreferencesDialog {...defaultProps({ onUpdateGameVersion })} />);
    switchToCategory("RimWorld");
    const select = screen.getByRole("combobox", { name: "Game version" }) as HTMLSelectElement;
    fireEvent.change(select, { target: { value: "1.5" } });
    await waitFor(() => expect(onUpdateGameVersion).toHaveBeenCalledWith("1.5"));
    expect(screen.queryByRole("alert")).toBeNull();
    expect(select.disabled).toBe(false);
  });

  it("does not call onUpdateGameVersion when the already-active version is re-selected", () => {
    const onUpdateGameVersion = vi.fn();
    render(<PreferencesDialog {...defaultProps({ onUpdateGameVersion })} />);
    switchToCategory("RimWorld");
    const select = screen.getByRole("combobox", { name: "Game version" });
    fireEvent.change(select, { target: { value: "1.6" } });
    expect(onUpdateGameVersion).not.toHaveBeenCalled();
  });

  it("disables the version selector while a version change is pending", async () => {
    let resolveChange!: () => void;
    const onUpdateGameVersion = vi.fn(
      () => new Promise<void>((resolve) => { resolveChange = resolve; }),
    );
    render(<PreferencesDialog {...defaultProps({ onUpdateGameVersion })} />);
    switchToCategory("RimWorld");
    const select = screen.getByRole("combobox", { name: "Game version" }) as HTMLSelectElement;
    fireEvent.change(select, { target: { value: "1.5" } });
    await waitFor(() => expect(select.disabled).toBe(true));
    resolveChange();
    await waitFor(() => expect(select.disabled).toBe(false));
  });

  it("confirms discarding dirty tabs before applying a version change", async () => {
    confirmDiscardChangesMock.mockClear();
    const onUpdateGameVersion = vi.fn().mockResolvedValue(undefined);
    render(
      <PreferencesDialog
        {...defaultProps({ onUpdateGameVersion, hasDirtyTabs: true })}
      />,
    );
    switchToCategory("RimWorld");
    const select = screen.getByRole("combobox", { name: "Game version" });
    fireEvent.change(select, { target: { value: "1.5" } });
    await waitFor(() => expect(confirmDiscardChangesMock).toHaveBeenCalled());
    await waitFor(() => expect(onUpdateGameVersion).toHaveBeenCalledWith("1.5"));
  });

  it("shows a dialog error banner when persistence fails", async () => {
    const onUpdateGameVersion = vi.fn().mockRejectedValueOnce({ message: "version save failed" });
    render(<PreferencesDialog {...defaultProps({ onUpdateGameVersion })} />);
    switchToCategory("RimWorld");
    const select = screen.getByRole("combobox", { name: "Game version" });
    fireEvent.change(select, { target: { value: "1.5" } });
    await waitFor(() => expect(screen.getByRole("alert")).toBeDefined());
    expect(screen.getByText("version save failed")).toBeDefined();
  });
});

describe("PreferencesDialog remove", () => {
  it("calls onRemoveLocation after confirm", async () => {
    confirmMock.mockResolvedValueOnce(true);
    const onRemoveLocation = vi.fn().mockResolvedValue(undefined);
    render(<PreferencesDialog {...defaultProps({ onRemoveLocation })} />);
    switchToCategory("Locations");
    fireEvent.click(screen.getAllByRole("button", { name: "Remove" })[0]);
    await waitFor(() => expect(onRemoveLocation).toHaveBeenCalled());
  });

  it("does not call onRemoveLocation when confirm is cancelled", async () => {
    confirmMock.mockResolvedValueOnce(false);
    const onRemoveLocation = vi.fn();
    render(<PreferencesDialog {...defaultProps({ onRemoveLocation })} />);
    switchToCategory("Locations");
    fireEvent.click(screen.getAllByRole("button", { name: "Remove" })[0]);
    await waitFor(() => expect(confirmMock).toHaveBeenCalled());
    expect(onRemoveLocation).not.toHaveBeenCalled();
  });
});

describe("PreferencesDialog accessibility", () => {
  it("opens with focus inside the dialog", () => {
    render(<PreferencesDialog {...defaultProps()} />);
    expect(screen.getByRole("dialog").contains(document.activeElement)).toBe(true);
  });

  it("closes on Escape", async () => {
    const onClose = vi.fn();
    const user = userEvent.setup();
    render(<PreferencesDialog {...defaultProps({ onClose })} />);
    await user.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("closes on the header close button", () => {
    const onClose = vi.fn();
    render(<PreferencesDialog {...defaultProps({ onClose })} />);
    const [headerClose] = screen.getAllByRole("button", { name: "Close" });
    fireEvent.click(headerClose);
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("closes on the footer close button", () => {
    const onClose = vi.fn();
    render(<PreferencesDialog {...defaultProps({ onClose })} />);
    const buttons = screen.getAllByRole("button", { name: "Close" });
    fireEvent.click(buttons[buttons.length - 1]);
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("closes on overlay click but not on panel click", () => {
    const onClose = vi.fn();
    render(<PreferencesDialog {...defaultProps({ onClose })} />);
    fireEvent.click(screen.getByRole("dialog"));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("does not close when clicking inside the panel", () => {
    const onClose = vi.fn();
    render(<PreferencesDialog {...defaultProps({ onClose })} />);
    fireEvent.click(screen.getByText("Preferences"));
    expect(onClose).not.toHaveBeenCalled();
  });

  it("restores focus to the invoking control on close", () => {
    const trigger = document.createElement("button");
    trigger.textContent = "open preferences";
    document.body.appendChild(trigger);
    trigger.focus();

    const { unmount } = render(<PreferencesDialog {...defaultProps()} />);
    expect(document.activeElement).not.toBe(trigger);
    unmount();
    expect(document.activeElement).toBe(trigger);
    trigger.remove();
  });

  it("Tab wraps focus within the dialog", async () => {
    const user = userEvent.setup();
    render(<PreferencesDialog {...defaultProps()} />);
    const focusable = screen.getAllByRole("button").concat(screen.getAllByRole("tab"));
    const last = focusable[focusable.length - 1];
    last.focus();
    await user.tab();
    expect(document.activeElement).not.toBe(document.body);
  });
});
