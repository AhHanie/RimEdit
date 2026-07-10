import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { SaveDefTemplateDialog } from "./SaveDefTemplateDialog";
import type { UserDefTemplate } from "../../types/defTemplates";

function makeTemplate(overrides: Partial<UserDefTemplate> = {}): UserDefTemplate {
  return {
    id: "tpl-1",
    defType: "ThingDef",
    name: "Weapon base",
    description: null,
    xml: "<ThingDef><defName>Gun_Autopistol</defName></ThingDef>",
    originalDefName: "Gun_Autopistol",
    originalLabel: "autopistol",
    sourceRelativePath: "Defs/Weapons.xml",
    gameVersion: "1.6",
    createdAt: "2026-07-05T00:00:00Z",
    updatedAt: "2026-07-05T00:00:00Z",
    ...overrides,
  };
}

describe("SaveDefTemplateDialog", () => {
  it("pre-fills the name input with the default name", () => {
    render(
      <SaveDefTemplateDialog
        defaultName="autopistol"
        onSave={vi.fn()}
        onClose={vi.fn()}
        onSaved={vi.fn()}
      />,
    );
    const input = screen.getByLabelText("Template name") as HTMLInputElement;
    expect(input.value).toBe("autopistol");
  });

  it("disables Save while the name is blank", async () => {
    render(
      <SaveDefTemplateDialog
        defaultName="autopistol"
        onSave={vi.fn()}
        onClose={vi.fn()}
        onSaved={vi.fn()}
      />,
    );
    const input = screen.getByLabelText("Template name");
    await userEvent.clear(input);
    const saveBtn = screen.getByText("Save") as HTMLButtonElement;
    expect(saveBtn.disabled).toBe(true);
  });

  it("calls onSave with the trimmed name and onSaved with the result", async () => {
    const template = makeTemplate();
    const onSave = vi.fn().mockResolvedValue(template);
    const onSaved = vi.fn();
    render(
      <SaveDefTemplateDialog
        defaultName="autopistol"
        onSave={onSave}
        onClose={vi.fn()}
        onSaved={onSaved}
      />,
    );
    const input = screen.getByLabelText("Template name");
    await userEvent.clear(input);
    await userEvent.type(input, "  Weapon base  ");
    await userEvent.click(screen.getByText("Save"));

    expect(onSave).toHaveBeenCalledWith("Weapon base");
    expect(onSaved).toHaveBeenCalledWith(template);
  });

  it("shows a friendly error and does not close when saving fails", async () => {
    const onSave = vi.fn().mockRejectedValue({ message: "Template name must not be blank." });
    const onSaved = vi.fn();
    render(
      <SaveDefTemplateDialog
        defaultName="autopistol"
        onSave={onSave}
        onClose={vi.fn()}
        onSaved={onSaved}
      />,
    );
    await userEvent.click(screen.getByText("Save"));

    expect(await screen.findByText("Template name must not be blank.")).toBeTruthy();
    expect(onSaved).not.toHaveBeenCalled();
  });

  it("calls onClose when Cancel is clicked", async () => {
    const onClose = vi.fn();
    render(
      <SaveDefTemplateDialog
        defaultName="autopistol"
        onSave={vi.fn()}
        onClose={onClose}
        onSaved={vi.fn()}
      />,
    );
    await userEvent.click(screen.getByText("Cancel"));
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
