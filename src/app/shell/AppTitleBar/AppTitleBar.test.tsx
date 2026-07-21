import { screen, fireEvent } from "@testing-library/react";
import { FolderOpen, FolderPlus, RefreshCw } from "lucide-react";
import { renderWithI18n as render } from "../../../i18n/testing/renderWithI18n";
import type { CommandAction, MenuDescriptor } from "../../commands/commandTypes";
import { AppTitleBar } from "./AppTitleBar";

function sampleCommands(): CommandAction[] {
  return [
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
  ];
}

function sampleMenus(): MenuDescriptor[] {
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
  ];
}

function defaultProps(overrides: Partial<Parameters<typeof AppTitleBar>[0]> = {}) {
  return {
    activeProjectName: null,
    activeProjectRoot: null,
    onOpenProject: vi.fn(),
    onAddSourceFolder: vi.fn(),
    onRefresh: vi.fn(),
    onTogglePalette: vi.fn(),
    onToggleExplorer: vi.fn(),
    explorerVisible: false,
    commands: sampleCommands(),
    menus: sampleMenus(),
    ...overrides,
  };
}

describe("AppTitleBar", () => {
  it("renders the Add source folder button", () => {
    render(<AppTitleBar {...defaultProps()} />);
    expect(screen.getByRole("button", { name: "Add source folder" })).toBeDefined();
  });

  it("calls onAddSourceFolder when the button is clicked", () => {
    const onAddSourceFolder = vi.fn();
    render(<AppTitleBar {...defaultProps({ onAddSourceFolder })} />);
    fireEvent.click(screen.getByRole("button", { name: "Add source folder" }));
    expect(onAddSourceFolder).toHaveBeenCalledOnce();
  });

  it("renders the menu bar in place of the brand block", () => {
    render(<AppTitleBar {...defaultProps()} />);
    expect(screen.getByRole("navigation", { name: "Menu Bar" })).toBeDefined();
    expect(screen.getByRole("button", { name: "File" })).toBeDefined();
    expect(screen.queryByText("RimEdit")).toBeNull();
  });

  it("no longer renders a theme-cycle control", () => {
    render(<AppTitleBar {...defaultProps()} />);
    expect(screen.queryByLabelText(/theme/i)).toBeNull();
  });
});
