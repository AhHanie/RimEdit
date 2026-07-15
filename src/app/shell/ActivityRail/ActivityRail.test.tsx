import { screen, fireEvent } from "@testing-library/react";
import { renderWithI18n as render } from "../../../i18n/testing/renderWithI18n";
import { ActivityRail } from "./ActivityRail";

function defaultProps(
  overrides: Partial<Parameters<typeof ActivityRail>[0]> = {},
) {
  return {
    activeView: null as Parameters<typeof ActivityRail>[0]["activeView"],
    onSelectView: vi.fn(),
    ...overrides,
  };
}

describe("ActivityRail", () => {
  it("renders all three nav buttons", () => {
    render(<ActivityRail {...defaultProps()} />);
    expect(screen.getByRole("button", { name: "Explorer" })).toBeDefined();
    expect(screen.getByRole("button", { name: "Search" })).toBeDefined();
    expect(screen.getByRole("button", { name: "Settings" })).toBeDefined();
  });

  it("Settings button is enabled", () => {
    render(<ActivityRail {...defaultProps()} />);
    const btn = screen.getByRole("button", { name: "Settings" }) as HTMLButtonElement;
    expect(btn.disabled).toBe(false);
  });

  it("calls onSelectView with 'settings' when Settings is clicked", () => {
    const onSelectView = vi.fn();
    render(<ActivityRail {...defaultProps({ onSelectView })} />);
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    expect(onSelectView).toHaveBeenCalledWith("settings");
  });

  it("Settings button has active styling when activeView is settings", () => {
    render(<ActivityRail {...defaultProps({ activeView: "settings" })} />);
    const btn = screen.getByRole("button", { name: "Settings" });
    expect(btn.className).toContain("btnActive");
  });

  it("Settings button does not have active styling when another view is active", () => {
    render(<ActivityRail {...defaultProps({ activeView: "explorer" })} />);
    const btn = screen.getByRole("button", { name: "Settings" });
    expect(btn.className).not.toContain("btnActive");
  });

  it("calls onSelectView with 'explorer' when Explorer is clicked", () => {
    const onSelectView = vi.fn();
    render(<ActivityRail {...defaultProps({ onSelectView })} />);
    fireEvent.click(screen.getByRole("button", { name: "Explorer" }));
    expect(onSelectView).toHaveBeenCalledWith("explorer");
  });
});
