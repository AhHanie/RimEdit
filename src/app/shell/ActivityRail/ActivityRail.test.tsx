import { screen, fireEvent } from "@testing-library/react";
import { renderWithI18n as render } from "../../../i18n/testing/renderWithI18n";
import { ActivityRail } from "./ActivityRail";

function defaultProps(
  overrides: Partial<Parameters<typeof ActivityRail>[0]> = {},
) {
  return {
    activeView: null as Parameters<typeof ActivityRail>[0]["activeView"],
    onSelectView: vi.fn(),
    onOpenPreferences: vi.fn(),
    ...overrides,
  };
}

describe("ActivityRail", () => {
  it("renders all three nav buttons", () => {
    render(<ActivityRail {...defaultProps()} />);
    expect(screen.getByRole("button", { name: "Explorer" })).toBeDefined();
    expect(screen.getByRole("button", { name: "Search" })).toBeDefined();
    expect(screen.getByRole("button", { name: "Preferences" })).toBeDefined();
  });

  it("Preferences button is enabled", () => {
    render(<ActivityRail {...defaultProps()} />);
    const btn = screen.getByRole("button", { name: "Preferences" }) as HTMLButtonElement;
    expect(btn.disabled).toBe(false);
  });

  it("calls onOpenPreferences when the gear is clicked", () => {
    const onOpenPreferences = vi.fn();
    render(<ActivityRail {...defaultProps({ onOpenPreferences })} />);
    fireEvent.click(screen.getByRole("button", { name: "Preferences" }));
    expect(onOpenPreferences).toHaveBeenCalledOnce();
  });

  it("Preferences button is never rendered as an activity pane / pressed state", () => {
    render(<ActivityRail {...defaultProps({ activeView: "explorer" })} />);
    const btn = screen.getByRole("button", { name: "Preferences" });
    expect(btn.className).not.toContain("btnActive");
    expect(btn.hasAttribute("aria-pressed")).toBe(false);
  });

  it("calls onSelectView with 'explorer' when Explorer is clicked", () => {
    const onSelectView = vi.fn();
    render(<ActivityRail {...defaultProps({ onSelectView })} />);
    fireEvent.click(screen.getByRole("button", { name: "Explorer" }));
    expect(onSelectView).toHaveBeenCalledWith("explorer");
  });
});
