import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { getVersion } from "@tauri-apps/api/app";
import { renderWithI18n as render } from "../../../i18n/testing/renderWithI18n";
import { AboutDialog } from "./AboutDialog";

vi.mock("@tauri-apps/api/app", () => ({ getVersion: vi.fn() }));

const getVersionMock = vi.mocked(getVersion);

describe("AboutDialog", () => {
  it("shows a loading state before the version resolves", () => {
    getVersionMock.mockReturnValue(new Promise(() => {}));
    render(<AboutDialog onClose={vi.fn()} />);
    expect(screen.getByText("Loading…")).toBeDefined();
  });

  it("shows the resolved packaged version", async () => {
    getVersionMock.mockResolvedValue("1.2.3");
    render(<AboutDialog onClose={vi.fn()} />);
    await waitFor(() => expect(screen.getByText("1.2.3")).toBeDefined());
  });

  it("falls back to an unavailable state when getVersion rejects", async () => {
    getVersionMock.mockRejectedValue(new Error("no ipc"));
    render(<AboutDialog onClose={vi.fn()} />);
    await waitFor(() => expect(screen.getByText("Unavailable")).toBeDefined());
  });

  it("has dialog semantics with an accessible name", () => {
    getVersionMock.mockReturnValue(new Promise(() => {}));
    render(<AboutDialog onClose={vi.fn()} />);
    const dialog = screen.getByRole("dialog", { name: "About RimEdit" });
    expect(dialog.getAttribute("aria-modal")).toBe("true");
  });

  it("calls onClose when a close button is clicked", async () => {
    // Both the header icon button and the footer button are labeled "Close".
    getVersionMock.mockReturnValue(new Promise(() => {}));
    const onClose = vi.fn();
    const user = userEvent.setup();
    render(<AboutDialog onClose={onClose} />);
    const [headerClose] = screen.getAllByRole("button", { name: "Close" });
    await user.click(headerClose);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("closes on Escape and restores focus to the previously focused element", async () => {
    getVersionMock.mockReturnValue(new Promise(() => {}));
    const onClose = vi.fn();
    const trigger = document.createElement("button");
    document.body.appendChild(trigger);
    trigger.focus();

    const { unmount } = render(<AboutDialog onClose={onClose} />);
    await userEvent.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalledTimes(1);

    unmount();
    expect(document.activeElement).toBe(trigger);
    trigger.remove();
  });
});
