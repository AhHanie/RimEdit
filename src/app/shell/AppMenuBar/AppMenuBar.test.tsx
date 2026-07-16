import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FolderOpen, FolderPlus, RefreshCw, Search, Settings, Sun, Moon, Monitor, Command, Info } from "lucide-react";
import { renderWithI18n as render } from "../../../i18n/testing/renderWithI18n";
import type { CommandAction, MenuDescriptor } from "../../commands/commandTypes";
import { AppMenuBar } from "./AppMenuBar";

function sampleCommands(overrides: Partial<Record<string, Partial<CommandAction>>> = {}): CommandAction[] {
  const base: CommandAction[] = [
    {
      id: "open-project",
      labelKey: "shell:commands.openProject.label",
      keywordsKey: "shell:commands.openProject.keywords",
      icon: FolderOpen,
      run: vi.fn(),
    },
    {
      id: "add-source-folder",
      labelKey: "shell:commands.addSourceFolder.label",
      keywordsKey: "shell:commands.addSourceFolder.keywords",
      icon: FolderPlus,
      run: vi.fn(),
    },
    {
      id: "refresh",
      labelKey: "shell:commands.refresh.label",
      keywordsKey: "shell:commands.refresh.keywords",
      icon: RefreshCw,
      run: vi.fn(),
      disabled: true,
    },
    {
      id: "open-command-palette",
      labelKey: "shell:commands.openCommandPalette.label",
      keywordsKey: "shell:commands.openCommandPalette.keywords",
      icon: Command,
      run: vi.fn(),
    },
    {
      id: "focus-search",
      labelKey: "shell:commands.focusSearch.label",
      keywordsKey: "shell:commands.focusSearch.keywords",
      icon: Search,
      run: vi.fn(),
    },
    {
      id: "toggle-explorer",
      labelKey: "shell:commands.toggleExplorer.label",
      keywordsKey: "shell:commands.toggleExplorer.keywords",
      icon: FolderOpen,
      run: vi.fn(),
    },
    {
      id: "open-settings",
      labelKey: "shell:commands.openSettings.label",
      keywordsKey: "shell:commands.openSettings.keywords",
      icon: Settings,
      run: vi.fn(),
    },
    {
      id: "theme-light",
      labelKey: "shell:commands.themeLight.label",
      keywordsKey: "shell:commands.themeLight.keywords",
      icon: Sun,
      run: vi.fn(),
    },
    {
      id: "theme-dark",
      labelKey: "shell:commands.themeDark.label",
      keywordsKey: "shell:commands.themeDark.keywords",
      icon: Moon,
      run: vi.fn(),
    },
    {
      id: "theme-system",
      labelKey: "shell:commands.themeSystem.label",
      keywordsKey: "shell:commands.themeSystem.keywords",
      icon: Monitor,
      run: vi.fn(),
    },
    {
      id: "show-about",
      labelKey: "shell:commands.showAbout.label",
      keywordsKey: "shell:commands.showAbout.keywords",
      icon: Info,
      run: vi.fn(),
    },
  ];
  return base.map((c) => ({ ...c, ...overrides[c.id] }));
}

function sampleMenus(explorerVisible = false, themeMode: "light" | "dark" | "system" = "system"): MenuDescriptor[] {
  return [
    {
      id: "file",
      labelKey: "shell:menuBar.file",
      entries: [
        { kind: "command", commandId: "open-project" },
        { kind: "command", commandId: "add-source-folder" },
        { kind: "separator" },
        { kind: "command", commandId: "refresh" },
      ],
    },
    {
      id: "view",
      labelKey: "shell:menuBar.view",
      entries: [
        { kind: "command", commandId: "open-command-palette" },
        { kind: "command", commandId: "focus-search" },
        { kind: "separator" },
        { kind: "command", commandId: "toggle-explorer", checked: explorerVisible },
        { kind: "command", commandId: "open-settings" },
      ],
    },
    {
      id: "theme",
      labelKey: "shell:menuBar.theme",
      entries: [
        { kind: "command", commandId: "theme-light", checked: themeMode === "light", radioGroup: true },
        { kind: "command", commandId: "theme-dark", checked: themeMode === "dark", radioGroup: true },
        { kind: "command", commandId: "theme-system", checked: themeMode === "system", radioGroup: true },
      ],
    },
    {
      id: "help",
      labelKey: "shell:menuBar.help",
      entries: [{ kind: "command", commandId: "show-about" }],
    },
  ];
}

describe("AppMenuBar", () => {
  it("renders the four top-level triggers with translated labels", () => {
    render(<AppMenuBar commands={sampleCommands()} menus={sampleMenus()} />);
    expect(screen.getByRole("navigation", { name: "Menu Bar" })).toBeDefined();
    for (const label of ["File", "View", "Theme", "Help"]) {
      expect(screen.getByRole("button", { name: label })).toBeDefined();
    }
  });

  it("opens the File menu on click and shows its commands", async () => {
    const user = userEvent.setup();
    render(<AppMenuBar commands={sampleCommands()} menus={sampleMenus()} />);
    await user.click(screen.getByRole("button", { name: "File" }));
    expect(screen.getByRole("menu")).toBeDefined();
    expect(screen.getByRole("menuitem", { name: "Open Project" })).toBeDefined();
    expect(screen.getByRole("menuitem", { name: "Add Source Folder" })).toBeDefined();
    expect(screen.getByRole("menuitem", { name: "Refresh Project Files" })).toBeDefined();
  });

  it("invokes the underlying command's run callback exactly once and closes the menu", async () => {
    const commands = sampleCommands();
    const openProject = commands.find((c) => c.id === "open-project")!;
    const user = userEvent.setup();
    render(<AppMenuBar commands={commands} menus={sampleMenus()} />);
    await user.click(screen.getByRole("button", { name: "File" }));
    await user.click(screen.getByRole("menuitem", { name: "Open Project" }));
    expect(openProject.run).toHaveBeenCalledOnce();
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("disables the Refresh command when its underlying command is disabled", async () => {
    const user = userEvent.setup();
    render(<AppMenuBar commands={sampleCommands()} menus={sampleMenus()} />);
    await user.click(screen.getByRole("button", { name: "File" }));
    expect(screen.getByRole("menuitem", { name: "Refresh Project Files" })).toHaveProperty("disabled", true);
  });

  it("only allows one popup open at a time", async () => {
    const user = userEvent.setup();
    render(<AppMenuBar commands={sampleCommands()} menus={sampleMenus()} />);
    await user.click(screen.getByRole("button", { name: "File" }));
    expect(screen.getAllByRole("menu")).toHaveLength(1);
    await user.click(screen.getByRole("button", { name: "View" }));
    expect(screen.getAllByRole("menu")).toHaveLength(1);
    expect(screen.getByRole("menuitem", { name: "Command Palette" })).toBeDefined();
  });

  it("clicking the active trigger again closes it", async () => {
    const user = userEvent.setup();
    render(<AppMenuBar commands={sampleCommands()} menus={sampleMenus()} />);
    const trigger = screen.getByRole("button", { name: "File" });
    await user.click(trigger);
    expect(screen.getByRole("menu")).toBeDefined();
    await user.click(trigger);
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("shows Explorer as checked while the explorer is visible", async () => {
    const user = userEvent.setup();
    render(<AppMenuBar commands={sampleCommands()} menus={sampleMenus(true)} />);
    await user.click(screen.getByRole("button", { name: "View" }));
    const explorerItem = screen.getByRole("menuitemcheckbox", { name: "Toggle Explorer" });
    expect(explorerItem.getAttribute("aria-checked")).toBe("true");
  });

  it("marks the active theme as checked via menuitemradio", async () => {
    const user = userEvent.setup();
    render(<AppMenuBar commands={sampleCommands()} menus={sampleMenus(false, "dark")} />);
    await user.click(screen.getByRole("button", { name: "Theme" }));
    expect(screen.getByRole("menuitemradio", { name: "Theme: Dark" }).getAttribute("aria-checked")).toBe(
      "true",
    );
    expect(screen.getByRole("menuitemradio", { name: "Theme: Light" }).getAttribute("aria-checked")).toBe(
      "false",
    );
  });

  it("invokes Help > About exactly once", async () => {
    const commands = sampleCommands();
    const showAbout = commands.find((c) => c.id === "show-about")!;
    const user = userEvent.setup();
    render(<AppMenuBar commands={commands} menus={sampleMenus()} />);
    await user.click(screen.getByRole("button", { name: "Help" }));
    await user.click(screen.getByRole("menuitem", { name: "About RimEdit" }));
    expect(showAbout.run).toHaveBeenCalledOnce();
  });

  it("closes the menu on outside click", async () => {
    const user = userEvent.setup();
    render(
      <div>
        <div data-testid="outside">outside</div>
        <AppMenuBar commands={sampleCommands()} menus={sampleMenus()} />
      </div>,
    );
    await user.click(screen.getByRole("button", { name: "File" }));
    expect(screen.getByRole("menu")).toBeDefined();
    await user.click(screen.getByTestId("outside"));
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("opens via ArrowDown, focuses the first item, and Escape restores focus to the trigger", async () => {
    const user = userEvent.setup();
    render(<AppMenuBar commands={sampleCommands()} menus={sampleMenus()} />);
    const trigger = screen.getByRole("button", { name: "File" });
    trigger.focus();
    await user.keyboard("{ArrowDown}");
    expect(document.activeElement).toBe(screen.getByRole("menuitem", { name: "Open Project" }));
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });

  it("navigates items with ArrowDown/ArrowUp, wrapping at the ends and skipping disabled entries", async () => {
    // "Refresh Project Files" is disabled in `sampleCommands`, so the File menu's enabled items
    // are just [Open Project, Add Source Folder] -- wrapping must skip over it in both directions.
    const user = userEvent.setup();
    render(<AppMenuBar commands={sampleCommands()} menus={sampleMenus()} />);
    screen.getByRole("button", { name: "File" }).focus();
    await user.keyboard("{ArrowDown}");
    expect(document.activeElement).toBe(screen.getByRole("menuitem", { name: "Open Project" }));
    await user.keyboard("{ArrowUp}");
    expect(document.activeElement).toBe(screen.getByRole("menuitem", { name: "Add Source Folder" }));
    await user.keyboard("{ArrowDown}");
    expect(document.activeElement).toBe(screen.getByRole("menuitem", { name: "Open Project" }));
  });

  it("switches to the adjacent menu with ArrowRight/ArrowLeft", async () => {
    const user = userEvent.setup();
    render(<AppMenuBar commands={sampleCommands()} menus={sampleMenus()} />);
    screen.getByRole("button", { name: "File" }).focus();
    await user.keyboard("{ArrowDown}");
    expect(screen.getByRole("menuitem", { name: "Open Project" })).toBeDefined();
    await user.keyboard("{ArrowRight}");
    expect(document.activeElement).toBe(screen.getByRole("menuitem", { name: "Command Palette" }));
    await user.keyboard("{ArrowLeft}");
    expect(document.activeElement).toBe(screen.getByRole("menuitem", { name: "Open Project" }));
  });
});
