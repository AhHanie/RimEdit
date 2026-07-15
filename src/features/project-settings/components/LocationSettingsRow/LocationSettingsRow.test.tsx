import { screen, fireEvent, waitFor } from "@testing-library/react";
import { renderWithI18n as render } from "../../../../i18n/testing/renderWithI18n";
import { LocationSettingsRow } from "./LocationSettingsRow";
import type { RegisteredLocation } from "../../types";

function makeLocation(overrides: Partial<RegisteredLocation> = {}): RegisteredLocation {
  return {
    id: "loc-1",
    displayName: "My Source",
    rootPath: "/some/path",
    kind: "source",
    sourceType: "folder",
    readOnly: true,
    modId: undefined,
    gameVersion: undefined,
    createdAt: "2024-01-01T00:00:00Z",
    updatedAt: "2024-01-01T00:00:00Z",
    ...overrides,
  };
}

function defaultProps(
  overrides: Partial<Parameters<typeof LocationSettingsRow>[0]> = {},
) {
  return {
    location: makeLocation(),
    isActive: false,
    onSave: vi.fn().mockResolvedValue(undefined),
    onRemove: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

describe("LocationSettingsRow view mode", () => {
  it("renders display name", () => {
    render(<LocationSettingsRow {...defaultProps()} />);
    expect(screen.getByText("My Source")).toBeDefined();
  });

  it("shows Active badge when isActive is true", () => {
    render(<LocationSettingsRow {...defaultProps({ isActive: true })} />);
    expect(screen.getByText("Active")).toBeDefined();
  });

  it("shows metadata fields when present", () => {
    render(
      <LocationSettingsRow
        {...defaultProps({
          location: makeLocation({ modId: "MyMod", gameVersion: "1.5" }),
        })}
      />,
    );
    expect(screen.getByText("mod: MyMod")).toBeDefined();
    expect(screen.getByText("version: 1.5")).toBeDefined();
  });

  it("does not show metadata section when all fields are absent", () => {
    render(<LocationSettingsRow {...defaultProps()} />);
    expect(screen.queryByText(/mod:/)).toBeNull();
  });
});

describe("LocationSettingsRow edit mode", () => {
  it("enters edit mode on pencil button click", () => {
    render(<LocationSettingsRow {...defaultProps()} />);
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    expect(screen.getByRole("textbox", { name: "Display name" })).toBeDefined();
  });

  it("pre-fills display name input with current value", () => {
    render(<LocationSettingsRow {...defaultProps()} />);
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    const input = screen.getByRole("textbox", { name: "Display name" }) as HTMLInputElement;
    expect(input.value).toBe("My Source");
  });

  it("shows source type select for source-kind rows", () => {
    render(<LocationSettingsRow {...defaultProps()} />);
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    expect(screen.getByRole("combobox", { name: "Source type" })).toBeDefined();
  });

  it("hides source type select for project-kind rows", () => {
    render(
      <LocationSettingsRow
        {...defaultProps({ location: makeLocation({ kind: "project", readOnly: false }) })}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    expect(screen.queryByRole("combobox", { name: "Source type" })).toBeNull();
  });

  it("shows mod ID input for source-kind rows", () => {
    render(<LocationSettingsRow {...defaultProps()} />);
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    expect(screen.getByRole("textbox", { name: "Mod ID" })).toBeDefined();
  });

  it("does not show expansion name input (expansion type removed)", () => {
    render(<LocationSettingsRow {...defaultProps()} />);
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    expect(screen.queryByRole("textbox", { name: "Expansion name" })).toBeNull();
  });

  it("save button is disabled when display name is blank", () => {
    render(<LocationSettingsRow {...defaultProps()} />);
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    const input = screen.getByRole("textbox", { name: "Display name" });
    fireEvent.change(input, { target: { value: "   " } });
    const saveBtn = screen.getByRole("button", { name: "Save" }) as HTMLButtonElement;
    expect(saveBtn.disabled).toBe(true);
  });

  it("save button is disabled when no fields are dirty", () => {
    render(<LocationSettingsRow {...defaultProps()} />);
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    const saveBtn = screen.getByRole("button", { name: "Save" }) as HTMLButtonElement;
    expect(saveBtn.disabled).toBe(true);
  });

  it("cancel returns to view mode without calling onSave", () => {
    render(<LocationSettingsRow {...defaultProps()} />);
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.getByRole("button", { name: "Edit" })).toBeDefined();
  });

  it("calls onSave with updated display name", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(<LocationSettingsRow {...defaultProps({ onSave })} />);
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    const input = screen.getByRole("textbox", { name: "Display name" });
    fireEvent.change(input, { target: { value: "New Name" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(onSave).toHaveBeenCalledOnce());
    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({ displayName: "New Name", id: "loc-1" }),
    );
  });

  it("returns to view mode after successful save", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(<LocationSettingsRow {...defaultProps({ onSave })} />);
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    const input = screen.getByRole("textbox", { name: "Display name" });
    fireEvent.change(input, { target: { value: "New Name" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Edit" })).toBeDefined());
  });

  it("stays in edit mode when onSave throws", async () => {
    const onSave = vi.fn().mockRejectedValue(new Error("server error"));
    render(<LocationSettingsRow {...defaultProps({ onSave })} />);
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    const input = screen.getByRole("textbox", { name: "Display name" });
    fireEvent.change(input, { target: { value: "New Name" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(onSave).toHaveBeenCalled());
    expect(screen.getByRole("textbox", { name: "Display name" })).toBeDefined();
  });

  it("passes modId in update", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(
      <LocationSettingsRow
        {...defaultProps({
          onSave,
          location: makeLocation({ sourceType: "localMod", modId: "OldMod" }),
        })}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    const nameInput = screen.getByRole("textbox", { name: "Display name" });
    fireEvent.change(nameInput, { target: { value: "Updated Name" } });
    const modInput = screen.getByRole("textbox", { name: "Mod ID" });
    fireEvent.change(modInput, { target: { value: "NewMod" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(onSave).toHaveBeenCalled());
    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({ modId: "NewMod", displayName: "Updated Name" }),
    );
  });
});
