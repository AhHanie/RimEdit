import { screen, waitFor } from "@testing-library/react";
import { renderWithI18n as render } from "../../../../i18n/testing/renderWithI18n";
import userEvent from "@testing-library/user-event";
import { FormViewSwitchConfirmDialog } from "./FormViewSwitchConfirmDialog";

describe("FormViewSwitchConfirmDialog", () => {
  it("offers discard/save-as-custom/cancel", () => {
    render(
      <FormViewSwitchConfirmDialog
        hiddenCount={2}
        onDiscardAndSwitch={vi.fn()}
        onSaveAsCustom={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    expect(screen.getByText("Discard changes and switch")).toBeTruthy();
    expect(screen.getByText("Save as custom view")).toBeTruthy();
    expect(screen.getByText("Cancel")).toBeTruthy();
    expect(screen.getByText(/2 hidden fields/)).toBeTruthy();
  });

  it("discards and switches", async () => {
    const onDiscardAndSwitch = vi.fn();
    render(
      <FormViewSwitchConfirmDialog
        hiddenCount={1}
        onDiscardAndSwitch={onDiscardAndSwitch}
        onSaveAsCustom={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    await userEvent.click(screen.getByText("Discard changes and switch"));
    expect(onDiscardAndSwitch).toHaveBeenCalledTimes(1);
  });

  it("cancels without calling either action", async () => {
    const onCancel = vi.fn();
    const onDiscardAndSwitch = vi.fn();
    render(
      <FormViewSwitchConfirmDialog
        hiddenCount={1}
        onDiscardAndSwitch={onDiscardAndSwitch}
        onSaveAsCustom={vi.fn()}
        onCancel={onCancel}
      />,
    );
    await userEvent.click(screen.getByText("Cancel"));
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(onDiscardAndSwitch).not.toHaveBeenCalled();
  });

  it("moves to a name step and calls onSaveAsCustom with the trimmed name", async () => {
    const onSaveAsCustom = vi.fn().mockResolvedValue(undefined);
    render(
      <FormViewSwitchConfirmDialog
        hiddenCount={1}
        onDiscardAndSwitch={vi.fn()}
        onSaveAsCustom={onSaveAsCustom}
        onCancel={vi.fn()}
      />,
    );
    await userEvent.click(screen.getByText("Save as custom view"));
    const input = screen.getByLabelText("Custom view name");
    await userEvent.type(input, "  My saved view  ");
    await userEvent.click(screen.getByText("Save and switch"));
    expect(onSaveAsCustom).toHaveBeenCalledWith("My saved view");
  });

  it("resets the busy state after onSaveAsCustom resolves, even when the caller does not unmount this dialog (finding 3 follow-up)", async () => {
    let resolveSave!: () => void;
    const savePromise = new Promise<void>((resolve) => {
      resolveSave = resolve;
    });
    const onSaveAsCustom = vi.fn().mockReturnValue(savePromise);
    render(
      <FormViewSwitchConfirmDialog
        hiddenCount={1}
        onDiscardAndSwitch={vi.fn()}
        onSaveAsCustom={onSaveAsCustom}
        onCancel={vi.fn()}
      />,
    );
    await userEvent.click(screen.getByText("Save as custom view"));
    await userEvent.type(screen.getByLabelText("Custom view name"), "My view");
    await userEvent.click(screen.getByText("Save and switch"));
    expect(screen.getByText("Saving…")).toBeTruthy();

    // `onSaveAsCustom` resolves, but (unlike the ordinary case) the caller deliberately does
    // NOT unmount this dialog -- e.g. because its own scope-staleness guard decided not to
    // dismiss it. `busy` must still reset, or Back/Save would stay disabled forever.
    resolveSave();
    await waitFor(() => expect(screen.queryByText("Saving…")).toBeNull());
    expect(screen.getByText("Save and switch")).toBeTruthy();
  });

  it("closes on Escape and restores focus to the previously focused element", async () => {
    const onCancel = vi.fn();
    const trigger = document.createElement("button");
    document.body.appendChild(trigger);
    trigger.focus();

    const { unmount } = render(
      <FormViewSwitchConfirmDialog
        hiddenCount={1}
        onDiscardAndSwitch={vi.fn()}
        onSaveAsCustom={vi.fn()}
        onCancel={onCancel}
      />,
    );
    await userEvent.keyboard("{Escape}");
    expect(onCancel).toHaveBeenCalledTimes(1);

    unmount();
    expect(document.activeElement).toBe(trigger);
    trigger.remove();
  });
});
