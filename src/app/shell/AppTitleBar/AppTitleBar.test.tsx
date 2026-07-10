import { render, screen, fireEvent } from "@testing-library/react";
import { AppTitleBar } from "./AppTitleBar";

function defaultProps(overrides: Partial<Parameters<typeof AppTitleBar>[0]> = {}) {
  return {
    activeProjectName: null,
    activeProjectRoot: null,
    themeMode: "system" as const,
    onCycleTheme: vi.fn(),
    onOpenProject: vi.fn(),
    onAddSourceFolder: vi.fn(),
    onRefresh: vi.fn(),
    onTogglePalette: vi.fn(),
    onToggleExplorer: vi.fn(),
    explorerVisible: false,
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
});
