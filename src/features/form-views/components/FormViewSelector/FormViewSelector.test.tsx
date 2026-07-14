import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FormViewSelector } from "./FormViewSelector";
import type { UseFormViewsResult } from "../../hooks/useFormViews";
import type { ResolvedFormView } from "../../types/resolvedFormView";

function view(overrides: Partial<ResolvedFormView> = {}): ResolvedFormView {
  return {
    id: "default",
    targetDefType: "ThingDef",
    label: "Default View",
    order: 0,
    origin: "default",
    hiddenFieldIds: [],
    recommended: false,
    ...overrides,
  };
}

function makeController(overrides: Partial<UseFormViewsResult> = {}): UseFormViewsResult {
  const defaultView = view();
  return {
    applicable: true,
    availableViews: [defaultView],
    selectedView: defaultView,
    effectiveHidden: new Set(),
    hiddenCount: 0,
    visibleTopLevelFieldIds: null,
    override: null,
    hasDirtyOverride: false,
    selectView: vi.fn(),
    selectDefaultView: vi.fn(),
    resetOverride: vi.fn(),
    setOverrideHiddenFieldIds: vi.fn(),
    saveOverrideAsCustomView: vi.fn().mockResolvedValue(undefined),
    duplicateAsCustomView: vi.fn().mockResolvedValue(undefined),
    createCustomView: vi.fn().mockResolvedValue(undefined),
    renameCustomView: vi.fn().mockResolvedValue(undefined),
    updateCustomView: vi.fn().mockResolvedValue(undefined),
    deleteCustomView: vi.fn().mockResolvedValue(undefined),
    getScopeKey: vi.fn().mockReturnValue("test-scope"),
    customViews: [],
    customViewsLoading: false,
    customViewsWarning: null,
    customViewsError: null,
    reloadCustomViews: vi.fn().mockResolvedValue(undefined),
    resetCustomViewStore: vi.fn().mockResolvedValue({ backupPath: null }),
    persistWarning: null,
    ...overrides,
  };
}

describe("FormViewSelector", () => {
  it("renders nothing when the controller reports not applicable", () => {
    const controller = makeController({ applicable: false });
    const { container } = render(
      <FormViewSelector controller={controller} onOpenManager={vi.fn()} />,
    );
    expect(container.firstChild).toBeNull();
  });

  it("lists Default first, then schema views (with a recommended/source marker), then custom views", () => {
    const defaultView = view();
    const weapon = view({
      id: "weapon",
      origin: "schema",
      label: "Weapon",
      recommended: true,
      source: { packId: "rimedit.core", packVersion: "1.6.0" },
    });
    const custom = view({ id: "custom-1", origin: "custom", label: "My view" });
    const controller = makeController({
      availableViews: [defaultView, weapon, custom],
      selectedView: weapon,
    });
    render(<FormViewSelector controller={controller} onOpenManager={vi.fn()} />);

    const select = screen.getByLabelText("View") as HTMLSelectElement;
    const optionLabels = Array.from(select.options).map((o) => o.textContent);
    expect(optionLabels).toEqual(["Default View", "Weapon (recommended)", "My view"]);
    expect(select.value).toBe("schema:weapon");
    expect(screen.getByText(/Schema view.*rimedit\.core 1\.6\.0/)).toBeTruthy();
  });

  it("always keeps Default View selectable", () => {
    const defaultView = view();
    const weapon = view({ id: "weapon", origin: "schema", label: "Weapon" });
    const controller = makeController({ availableViews: [defaultView, weapon], selectedView: weapon });
    render(<FormViewSelector controller={controller} onOpenManager={vi.fn()} />);
    const select = screen.getByLabelText("View") as HTMLSelectElement;
    expect(Array.from(select.options).some((o) => o.value === "default:default")).toBe(true);
  });

  it("switches immediately (no confirmation) when there is no dirty override", async () => {
    const defaultView = view();
    const weapon = view({ id: "weapon", origin: "schema", label: "Weapon" });
    const controller = makeController({ availableViews: [defaultView, weapon], selectedView: defaultView });
    render(<FormViewSelector controller={controller} onOpenManager={vi.fn()} />);

    await userEvent.selectOptions(screen.getByLabelText("View"), "schema:weapon");
    expect(controller.selectView).toHaveBeenCalledWith({ origin: "schema", id: "weapon" });
    expect(screen.queryByText("Unsaved view changes")).toBeNull();
  });

  it("prompts discard/save-as-custom/cancel when switching with a dirty override", async () => {
    const defaultView = view();
    const weapon = view({ id: "weapon", origin: "schema", label: "Weapon" });
    const controller = makeController({
      availableViews: [defaultView, weapon],
      selectedView: defaultView,
      hasDirtyOverride: true,
      hiddenCount: 2,
      override: { hiddenFieldIds: new Set(["apparel", "plant"]), isDirty: true },
    });
    render(<FormViewSelector controller={controller} onOpenManager={vi.fn()} />);

    await userEvent.selectOptions(screen.getByLabelText("View"), "schema:weapon");
    expect(controller.selectView).not.toHaveBeenCalled();
    expect(screen.getByText("Unsaved view changes")).toBeTruthy();

    await userEvent.click(screen.getByText("Discard changes and switch"));
    expect(controller.selectView).toHaveBeenCalledWith({ origin: "schema", id: "weapon" });
  });

  it("shows a Modified indicator with Reset/Discard when an override is dirty", async () => {
    const controller = makeController({
      hasDirtyOverride: true,
      hiddenCount: 3,
      override: { hiddenFieldIds: new Set(["a", "b", "c"]), isDirty: true },
    });
    render(<FormViewSelector controller={controller} onOpenManager={vi.fn()} />);

    expect(screen.getByText("Modified · 3 hidden")).toBeTruthy();
    await userEvent.click(screen.getByText("Reset"));
    expect(controller.resetOverride).toHaveBeenCalledTimes(1);
    await userEvent.click(screen.getByText("Discard"));
    expect(controller.resetOverride).toHaveBeenCalledTimes(2);
  });

  it("disables Show full form only when Default is already selected with no override", async () => {
    const controller = makeController();
    render(<FormViewSelector controller={controller} onOpenManager={vi.fn()} />);
    expect(screen.getByText("Show full form")).toHaveProperty("disabled", true);
  });

  it("Show full form switches to Default when not already selected", async () => {
    const defaultView = view();
    const weapon = view({ id: "weapon", origin: "schema", label: "Weapon" });
    const controller = makeController({ availableViews: [defaultView, weapon], selectedView: weapon });
    render(<FormViewSelector controller={controller} onOpenManager={vi.fn()} />);

    await userEvent.click(screen.getByText("Show full form"));
    expect(controller.selectView).toHaveBeenCalledWith({ origin: "default", id: "default" });
  });

  it("opens the manager when Customize view is clicked", async () => {
    const onOpenManager = vi.fn();
    const controller = makeController();
    render(<FormViewSelector controller={controller} onOpenManager={onOpenManager} />);
    await userEvent.click(screen.getByText("Customize view"));
    expect(onOpenManager).toHaveBeenCalledTimes(1);
  });

  it("shows a persist warning when the controller reports one, even with no dirty override and the manager closed", () => {
    const controller = makeController({
      persistWarning: "Could not save your Form View selection.",
    });
    render(<FormViewSelector controller={controller} onOpenManager={vi.fn()} />);
    expect(screen.getByRole("alert").textContent).toBe(
      "Could not save your Form View selection.",
    );
  });

  it("shows no persist warning when the controller reports none", () => {
    const controller = makeController({ persistWarning: null });
    render(<FormViewSelector controller={controller} onOpenManager={vi.fn()} />);
    expect(screen.queryByRole("alert")).toBeNull();
  });
});

// --- Issue 08: hidden validation feedback header/reveal UI ---------------------------------

describe("FormViewSelector - hidden field issues summary and reveal (issue 08)", () => {
  it("renders no summary/reveal affordance when hiddenIssues is omitted", () => {
    const controller = makeController();
    render(<FormViewSelector controller={controller} onOpenManager={vi.fn()} />);
    expect(screen.queryByText(/hidden field issue/)).toBeNull();
    expect(screen.queryByText("Reveal fields with issues")).toBeNull();
  });

  it("renders no summary/reveal affordance when hiddenIssues has zero total count", () => {
    const controller = makeController();
    render(
      <FormViewSelector
        controller={controller}
        onOpenManager={vi.fn()}
        hiddenIssues={{ affectedRootIds: new Set(), totalCount: 0, blockingCount: 0 }}
        onReveal={vi.fn()}
      />,
    );
    expect(screen.queryByText(/hidden field issue/)).toBeNull();
    expect(screen.queryByText("Reveal fields with issues")).toBeNull();
  });

  it("renders the total count with no blocking qualifier when nothing is blocking", () => {
    const controller = makeController();
    render(
      <FormViewSelector
        controller={controller}
        onOpenManager={vi.fn()}
        hiddenIssues={{ affectedRootIds: new Set(["graphicData"]), totalCount: 2, blockingCount: 0 }}
        onReveal={vi.fn()}
      />,
    );
    expect(screen.getByText("2 hidden field issues")).toBeTruthy();
  });

  it("renders singular wording for exactly one issue", () => {
    const controller = makeController();
    render(
      <FormViewSelector
        controller={controller}
        onOpenManager={vi.fn()}
        hiddenIssues={{ affectedRootIds: new Set(["graphicData"]), totalCount: 1, blockingCount: 0 }}
        onReveal={vi.fn()}
      />,
    );
    expect(screen.getByText("1 hidden field issue")).toBeTruthy();
  });

  it("renders the blocking count as accessible text, not color alone", () => {
    const controller = makeController();
    render(
      <FormViewSelector
        controller={controller}
        onOpenManager={vi.fn()}
        hiddenIssues={{ affectedRootIds: new Set(["graphicData"]), totalCount: 3, blockingCount: 1 }}
        onReveal={vi.fn()}
      />,
    );
    expect(screen.getByText("3 hidden field issues (1 blocking)")).toBeTruthy();
  });

  it("calls onReveal when Reveal fields with issues is clicked, without touching selectView/setOverrideHiddenFieldIds itself", async () => {
    const onReveal = vi.fn();
    const controller = makeController();
    render(
      <FormViewSelector
        controller={controller}
        onOpenManager={vi.fn()}
        hiddenIssues={{ affectedRootIds: new Set(["graphicData"]), totalCount: 1, blockingCount: 1 }}
        onReveal={onReveal}
      />,
    );
    await userEvent.click(screen.getByText("Reveal fields with issues"));
    expect(onReveal).toHaveBeenCalledTimes(1);
    // The button itself never calls the controller directly -- the caller (XmlFormEditor) owns
    // computing/applying the override; FormViewSelector is purely presentational here.
    expect(controller.setOverrideHiddenFieldIds).not.toHaveBeenCalled();
    expect(controller.selectView).not.toHaveBeenCalled();
  });
});
