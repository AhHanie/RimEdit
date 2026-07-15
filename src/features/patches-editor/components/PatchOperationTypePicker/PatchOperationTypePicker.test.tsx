import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { renderWithI18n as render } from "../../../../i18n/testing/renderWithI18n";
import { PatchOperationTypePicker } from "./PatchOperationTypePicker";
import type { SchemaCatalog } from "../../../schema-catalog/types";

function catalogWithCustomOp(): SchemaCatalog {
  return {
    formatVersion: 1,
    packs: [],
    defTypes: {},
    objectTypes: {},
    patchOperations: {
      "MyMod.PatchOperationFoo": {
        className: "MyMod.PatchOperationFoo",
        label: "Foo Operation",
        fieldOrder: [],
        fields: {},
        preview: { kind: "unsupported" },
      },
    },
  };
}

describe("PatchOperationTypePicker", () => {
  it("lists every built-in operation class when the catalog has no custom operations", () => {
    render(<PatchOperationTypePicker catalog={null} onSelect={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.getByRole("button", { name: /PatchOperationRemove/i })).toBeTruthy();
    expect(screen.getByRole("button", { name: /PatchOperationSequence/i })).toBeTruthy();
    expect(screen.getByRole("button", { name: /PatchOperationFindMod/i })).toBeTruthy();
  });

  it("also lists metadata-defined custom operations, marked as custom", () => {
    render(
      <PatchOperationTypePicker catalog={catalogWithCustomOp()} onSelect={vi.fn()} onCancel={vi.fn()} />,
    );
    const option = screen.getByRole("button", { name: /Foo Operation/i });
    expect(option).toBeTruthy();
    expect(option.textContent).toContain("custom");
  });

  it("filters options by typed query against class name or label", async () => {
    render(
      <PatchOperationTypePicker catalog={catalogWithCustomOp()} onSelect={vi.fn()} onCancel={vi.fn()} />,
    );
    await userEvent.type(screen.getByPlaceholderText("Search operation type…"), "Foo");

    expect(screen.getByRole("button", { name: /Foo Operation/i })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /PatchOperationAdd/i })).toBeNull();
  });

  it("shows an empty state when no option matches the query", async () => {
    render(<PatchOperationTypePicker catalog={null} onSelect={vi.fn()} onCancel={vi.fn()} />);
    await userEvent.type(
      screen.getByPlaceholderText("Search operation type…"),
      "NoSuchOperationAnywhere",
    );
    expect(screen.getByText("No matching operation type.")).toBeTruthy();
  });

  it("calls onSelect with the class name when an option is clicked", async () => {
    const onSelect = vi.fn();
    render(<PatchOperationTypePicker catalog={null} onSelect={onSelect} onCancel={vi.fn()} />);
    await userEvent.click(screen.getByRole("button", { name: /PatchOperationRemove/i }));
    expect(onSelect).toHaveBeenCalledWith("PatchOperationRemove");
  });

  it("calls onCancel when Cancel is clicked", async () => {
    const onCancel = vi.fn();
    render(<PatchOperationTypePicker catalog={null} onSelect={vi.fn()} onCancel={onCancel} />);
    await userEvent.click(screen.getByText("Cancel"));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });
});
