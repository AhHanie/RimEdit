import { screen, waitFor } from "@testing-library/react";
import { renderWithI18n as render } from "../../../../i18n/testing/renderWithI18n";
import userEvent from "@testing-library/user-event";
import { confirm } from "@tauri-apps/plugin-dialog";
import { FormViewManagerDialog, type FormViewFieldChecklistTarget } from "./FormViewManagerDialog";
import type { UseFormViewsResult } from "../../hooks/useFormViews";
import type { ResolvedFormView } from "../../types/resolvedFormView";
import type { DefEditorView } from "../../../xml-editor/types/xmlDocument";
import type { SchemaCatalog } from "../../../schema-catalog";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: vi.fn(),
}));
const confirmMock = vi.mocked(confirm);

function makeFieldChecklistTarget(): FormViewFieldChecklistTarget {
  const catalog: SchemaCatalog = {
    formatVersion: 1,
    packs: [],
    objectTypes: {},
    defTypes: {
      ThingDef: {
        inherits: [],
        abstractType: false,
        fieldOrder: ["defName"],
        fields: {
          defName: {
            type: { kind: "string" },
            required: false,
            examples: [],
            repeatable: false,
            xml: "element",
            flags: false,
          },
        },
      },
    },
  };
  const def: DefEditorView = {
    nodeId: 1,
    defType: "ThingDef",
    defName: "Test",
    label: null,
    parentName: null,
    line: null,
    column: null,
    attributes: [],
    children: [],
  };
  return { def, defSchema: catalog.defTypes.ThingDef, catalog };
}

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
    duplicateAsCustomView: vi.fn().mockResolvedValue(view({ id: "dup", origin: "custom" })),
    createCustomView: vi.fn().mockResolvedValue(view({ id: "new", origin: "custom" })),
    renameCustomView: vi.fn().mockResolvedValue(view({ id: "custom-1", origin: "custom" })),
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

describe("FormViewManagerDialog", () => {
  beforeEach(() => {
    confirmMock.mockReset();
  });

  it("shows Default as read-only with a Duplicate action but no rename/delete", async () => {
    const controller = makeController();
    render(<FormViewManagerDialog controller={controller} onClose={vi.fn()} />);

    expect(screen.getByText("Always available · read-only")).toBeTruthy();
    expect(screen.getByText("Duplicate")).toBeTruthy();
    expect(screen.queryByLabelText("Rename Default View")).toBeNull();
    expect(screen.queryByText("Delete")).toBeNull();
  });

  it("shows a schema view's source/read-only annotation with only a Duplicate action", () => {
    const defaultView = view();
    const weapon = view({
      id: "weapon",
      origin: "schema",
      label: "Weapon",
      recommended: true,
      source: { packId: "rimedit.core", packVersion: "1.6.0" },
    });
    const controller = makeController({ availableViews: [defaultView, weapon] });
    render(<FormViewManagerDialog controller={controller} onClose={vi.fn()} />);

    expect(screen.getByText("Schema pack · rimedit.core 1.6.0 · read-only")).toBeTruthy();
    expect(screen.getByText("Recommended")).toBeTruthy();
    expect(screen.queryByLabelText("Rename Weapon")).toBeNull();
  });

  it("duplicates a schema/Default view into a new custom view", async () => {
    const defaultView = view();
    const weapon = view({ id: "weapon", origin: "schema", label: "Weapon" });
    const controller = makeController({ availableViews: [defaultView, weapon] });
    render(<FormViewManagerDialog controller={controller} onClose={vi.fn()} />);

    const duplicateButtons = screen.getAllByText("Duplicate");
    await userEvent.click(duplicateButtons[1]); // the schema row's Duplicate
    expect(controller.duplicateAsCustomView).toHaveBeenCalledWith(weapon);
  });

  it("offers rename and delete only for custom views", async () => {
    const defaultView = view();
    const custom = view({ id: "custom-1", origin: "custom", label: "My view" });
    const controller = makeController({ availableViews: [defaultView, custom] });
    render(<FormViewManagerDialog controller={controller} onClose={vi.fn()} />);

    expect(screen.getByLabelText("Rename My view")).toBeTruthy();
    expect(screen.getByText("Delete")).toBeTruthy();
  });

  // Plan.md section 6/12: "A missing/renamed base becomes a nonblocking 'derived from
  // unavailable view' notice, not a broken view." / "Show unavailable-base/missing-field
  // notices in manager."
  it("shows an unavailable-base notice for a custom view whose recorded base view was renamed/removed", () => {
    const defaultView = view();
    // No schema-origin view named "weapon" is in `availableViews` at all -- standing in for a
    // pack upgrade that renamed or removed the view this custom view was originally duplicated
    // from.
    const custom = view({
      id: "custom-1",
      origin: "custom",
      label: "My weapon view",
      baseSchemaView: {
        viewId: "weapon",
        packId: "rimedit.core",
        packVersion: "1.6.0",
        declaredOnDefType: "ThingDef",
      },
    });
    const controller = makeController({ availableViews: [defaultView, custom] });
    render(<FormViewManagerDialog controller={controller} onClose={vi.fn()} />);

    expect(
      screen.getByText(/derived from a view that.s no longer available/i),
    ).toBeTruthy();
  });

  it("shows no unavailable-base notice when the custom view's recorded base still resolves", () => {
    const defaultView = view();
    const weapon = view({ id: "weapon", origin: "schema", label: "Weapon" });
    const custom = view({
      id: "custom-1",
      origin: "custom",
      label: "My weapon view",
      baseSchemaView: {
        viewId: "weapon",
        packId: "rimedit.core",
        packVersion: "1.6.0",
        declaredOnDefType: "ThingDef",
      },
    });
    const controller = makeController({ availableViews: [defaultView, weapon, custom] });
    render(<FormViewManagerDialog controller={controller} onClose={vi.fn()} />);

    expect(screen.queryByText(/no longer available/i)).toBeNull();
  });

  it("shows no unavailable-base notice for a custom view with no recorded base at all", () => {
    const defaultView = view();
    const custom = view({ id: "custom-1", origin: "custom", label: "My view" });
    const controller = makeController({ availableViews: [defaultView, custom] });
    render(<FormViewManagerDialog controller={controller} onClose={vi.fn()} />);

    expect(screen.queryByText(/no longer available/i)).toBeNull();
  });

  it("renames a custom view", async () => {
    const defaultView = view();
    const custom = view({ id: "custom-1", origin: "custom", label: "My view" });
    const controller = makeController({ availableViews: [defaultView, custom] });
    render(<FormViewManagerDialog controller={controller} onClose={vi.fn()} />);

    await userEvent.click(screen.getByLabelText("Rename My view"));
    const input = screen.getByLabelText("Rename My view") as HTMLInputElement;
    await userEvent.clear(input);
    await userEvent.type(input, "Renamed view");
    await userEvent.click(screen.getByLabelText("Confirm rename"));

    expect(controller.renameCustomView).toHaveBeenCalledWith("custom-1", "Renamed view");
  });

  it("deletes a custom view only after confirmation", async () => {
    const defaultView = view();
    const custom = view({ id: "custom-1", origin: "custom", label: "My view" });
    const controller = makeController({ availableViews: [defaultView, custom] });
    render(<FormViewManagerDialog controller={controller} onClose={vi.fn()} />);

    confirmMock.mockResolvedValueOnce(false);
    await userEvent.click(screen.getByText("Delete"));
    expect(controller.deleteCustomView).not.toHaveBeenCalled();

    confirmMock.mockResolvedValueOnce(true);
    await userEvent.click(screen.getByText("Delete"));
    expect(controller.deleteCustomView).toHaveBeenCalledWith("custom-1");
  });

  it("creates a new custom view from the name field and selects it directly when there is no dirty override", async () => {
    const controller = makeController();
    render(<FormViewManagerDialog controller={controller} onClose={vi.fn()} />);

    await userEvent.type(screen.getByLabelText("New custom view name"), "Fresh view");
    await userEvent.click(screen.getByText("Create"));
    expect(controller.createCustomView).toHaveBeenCalledWith("Fresh view");
    // Clean state: no confirmation needed, the new view is selected immediately.
    expect(controller.selectView).toHaveBeenCalledWith({ origin: "custom", id: "new" });
    expect(screen.queryByText("Unsaved view changes")).toBeNull();
  });

  it("creating a new custom view while an override is dirty prompts instead of silently discarding it", async () => {
    const controller = makeController({
      hasDirtyOverride: true,
      hiddenCount: 2,
      override: { hiddenFieldIds: new Set(["apparel", "plant"]), isDirty: true },
    });
    render(<FormViewManagerDialog controller={controller} onClose={vi.fn()} />);

    await userEvent.type(screen.getByLabelText("New custom view name"), "Fresh view");
    await userEvent.click(screen.getByText("Create"));

    expect(controller.createCustomView).toHaveBeenCalledWith("Fresh view");
    // The new view must NOT be silently selected -- that would discard the dirty override
    // without asking. Instead the same three-way confirmation as any other switch appears.
    expect(controller.selectView).not.toHaveBeenCalled();
    expect(screen.getByText("Unsaved view changes")).toBeTruthy();

    await userEvent.click(screen.getByText("Discard changes and switch"));
    expect(controller.selectView).toHaveBeenCalledWith({ origin: "custom", id: "new" });
  });

  it("switches to a view via 'Use this view', gated by the same dirty-override confirmation", async () => {
    const defaultView = view();
    const weapon = view({ id: "weapon", origin: "schema", label: "Weapon" });
    const controller = makeController({
      availableViews: [defaultView, weapon],
      hasDirtyOverride: true,
      hiddenCount: 1,
      override: { hiddenFieldIds: new Set(["apparel"]), isDirty: true },
    });
    render(<FormViewManagerDialog controller={controller} onClose={vi.fn()} />);

    const useButtons = screen.getAllByText("Use this view");
    await userEvent.click(useButtons[1]); // Weapon's row
    expect(controller.selectView).not.toHaveBeenCalled();
    expect(screen.getByText("Unsaved view changes")).toBeTruthy();
  });

  it("Escape on a nested switch-confirmation only closes that dialog, not the manager underneath", async () => {
    const defaultView = view();
    const weapon = view({ id: "weapon", origin: "schema", label: "Weapon" });
    const onClose = vi.fn();
    const controller = makeController({
      availableViews: [defaultView, weapon],
      hasDirtyOverride: true,
      hiddenCount: 1,
      override: { hiddenFieldIds: new Set(["apparel"]), isDirty: true },
    });
    render(<FormViewManagerDialog controller={controller} onClose={onClose} />);

    const useButtons = screen.getAllByText("Use this view");
    await userEvent.click(useButtons[1]);
    expect(screen.getByText("Unsaved view changes")).toBeTruthy();

    await userEvent.keyboard("{Escape}");

    expect(screen.queryByText("Unsaved view changes")).toBeNull();
    expect(screen.getByText("Manage Form Views")).toBeTruthy();
    expect(onClose).not.toHaveBeenCalled();
  });

  it("closes on Escape and restores focus to the previously focused element", async () => {
    const onClose = vi.fn();
    const controller = makeController();
    const trigger = document.createElement("button");
    trigger.textContent = "open";
    document.body.appendChild(trigger);
    trigger.focus();
    expect(document.activeElement).toBe(trigger);

    const { unmount } = render(
      <FormViewManagerDialog controller={controller} onClose={onClose} />,
    );

    await userEvent.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalledTimes(1);

    unmount();
    expect(document.activeElement).toBe(trigger);
    trigger.remove();
  });

  it("moves focus into the dialog on open and traps Tab within it", async () => {
    const controller = makeController();
    const trigger = document.createElement("button");
    trigger.textContent = "open";
    document.body.appendChild(trigger);
    trigger.focus();
    expect(document.activeElement).toBe(trigger);

    render(<FormViewManagerDialog controller={controller} onClose={vi.fn()} />);

    // Focus must move off the trigger and into the dialog as soon as it opens -- otherwise the
    // Tab-wrap logic never engages because it only wraps once focus is already on the first/
    // last focusable element inside the dialog.
    expect(document.activeElement).not.toBe(trigger);
    expect(document.activeElement).not.toBe(document.body);
    const dialog = screen.getByRole("dialog");
    expect(dialog.contains(document.activeElement)).toBe(true);

    const focusable = Array.from(
      dialog.querySelectorAll<HTMLElement>(
        'button:not(:disabled), input:not(:disabled), [href], select, textarea',
      ),
    );
    expect(focusable.length).toBeGreaterThan(1);
    const last = focusable[focusable.length - 1];
    last.focus();
    expect(document.activeElement).toBe(last);

    await userEvent.tab();
    // Tabbing past the last focusable element wraps back to the first, rather than escaping
    // to the background page (the trigger button).
    expect(document.activeElement).toBe(focusable[0]);
    expect(document.activeElement).not.toBe(trigger);

    trigger.remove();
  });

  it("surfaces a custom-view store load error visibly with working Retry and Reset store actions", async () => {
    const reloadCustomViews = vi.fn().mockResolvedValue(undefined);
    const resetCustomViewStore = vi.fn().mockResolvedValue({ backupPath: "/backup/form-views.bak" });
    const controller = makeController({
      customViewsError: "store read failed: permission denied",
      reloadCustomViews,
      resetCustomViewStore,
    });
    render(<FormViewManagerDialog controller={controller} onClose={vi.fn()} />);

    // The error must be visible (not just logged/swallowed), distinct from the ordinary
    // per-action `error`/`warningBanner` styling, and paired with real recovery actions.
    expect(screen.getByRole("alert").textContent).toMatch(/store read failed: permission denied/);
    expect(screen.getByText("Retry")).toBeTruthy();
    expect(screen.getByText("Reset store")).toBeTruthy();

    await userEvent.click(screen.getByText("Retry"));
    expect(reloadCustomViews).toHaveBeenCalledTimes(1);

    confirmMock.mockResolvedValueOnce(true);
    await userEvent.click(screen.getByText("Reset store"));
    expect(resetCustomViewStore).toHaveBeenCalledTimes(1);
    expect(await screen.findByText(/backed up to \/backup\/form-views\.bak/)).toBeTruthy();
  });

  it("does not reset the custom-view store without confirmation", async () => {
    const resetCustomViewStore = vi.fn().mockResolvedValue({ backupPath: null });
    const controller = makeController({
      customViewsError: "store read failed",
      resetCustomViewStore,
    });
    render(<FormViewManagerDialog controller={controller} onClose={vi.fn()} />);

    confirmMock.mockResolvedValueOnce(false);
    await userEvent.click(screen.getByText("Reset store"));
    expect(resetCustomViewStore).not.toHaveBeenCalled();
  });

  it("renders no store-error banner when there is no load error", () => {
    const controller = makeController({ customViewsError: null });
    render(<FormViewManagerDialog controller={controller} onClose={vi.fn()} />);
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.queryByText("Reset store")).toBeNull();
  });

  it("surfaces a failed selection-persist warning visibly", () => {
    const controller = makeController({
      persistWarning: "Could not save your Form View selection.",
    });
    render(<FormViewManagerDialog controller={controller} onClose={vi.fn()} />);
    expect(screen.getByRole("alert").textContent).toBe(
      "Could not save your Form View selection.",
    );
  });

  it("renders no persist-warning banner when there is none", () => {
    const controller = makeController({ persistWarning: null });
    render(<FormViewManagerDialog controller={controller} onClose={vi.fn()} />);
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("renders a custom-view-store warning through the shared diagnostic catalog, not the raw backend message", () => {
    const controller = makeController({
      customViewsWarning: {
        code: "form_view_unsupported_version",
        // Deliberately different from the catalog text below -- proves the banner renders the
        // translated code/args lookup (`renderDiagnostic`'s priority-1 path), not a pass-through
        // of this compatibility message.
        message: "The custom Form View store was saved by a newer version of RimEdit.",
        args: { schemaVersion: 7 },
      },
    });
    render(<FormViewManagerDialog controller={controller} onClose={vi.fn()} />);
    expect(
      screen.getByText(
        "This Form View store was saved by a newer version of RimEdit (schema version 7).",
      ),
    ).toBeTruthy();
  });

  describe("available-views scroll region (Form View Manager scrollable list)", () => {
    it("renders every schema/custom view in order inside a named region separate from the field checklist", () => {
      const defaultView = view();
      const schemaViews = Array.from({ length: 4 }, (_, i) =>
        view({ id: `schema-${i}`, origin: "schema", label: `Schema View ${i}` }),
      );
      const customViews = Array.from({ length: 4 }, (_, i) =>
        view({ id: `custom-${i}`, origin: "custom", label: `Custom View ${i}` }),
      );
      const controller = makeController({
        availableViews: [defaultView, ...schemaViews, ...customViews],
      });
      render(
        <FormViewManagerDialog
          controller={controller}
          onClose={vi.fn()}
          fieldChecklistTarget={makeFieldChecklistTarget()}
        />,
      );

      const list = screen.getByRole("list", { name: "Available form views" });
      const rowLabels = Array.from(list.querySelectorAll("li")).map((li) => li.textContent);
      expect(rowLabels).toHaveLength(1 + schemaViews.length + customViews.length);
      expect(rowLabels[0]).toContain("Default View");
      schemaViews.forEach((v, i) => expect(rowLabels[1 + i]).toContain(v.label));
      customViews.forEach((v, i) => expect(rowLabels[1 + schemaViews.length + i]).toContain(v.label));

      // The field checklist's own list must stay a distinct region, not nested inside the
      // available-views list.
      const checklist = screen.getByLabelText("Filter fields");
      expect(list.contains(checklist)).toBe(false);
      expect(checklist.closest("ul")).toBeNull();
    });
  });

  describe("field checklist (issue 07)", () => {
    it("renders the field checklist when a target is supplied", () => {
      const controller = makeController();
      render(
        <FormViewManagerDialog
          controller={controller}
          onClose={vi.fn()}
          fieldChecklistTarget={makeFieldChecklistTarget()}
        />,
      );
      expect(screen.getByLabelText("Filter fields")).toBeTruthy();
      expect(screen.getByLabelText("defName")).toBeTruthy();
    });

    it("falls back to the placeholder when no target is supplied", () => {
      const controller = makeController();
      render(<FormViewManagerDialog controller={controller} onClose={vi.fn()} />);
      expect(screen.queryByLabelText("Filter fields")).toBeNull();
      expect(screen.getByText(/isn't available for the current selection/)).toBeTruthy();
    });
  });

  describe("close-with-dirty-override (issue 07 step 6)", () => {
    it("closes immediately when there is no dirty override", async () => {
      const onClose = vi.fn();
      const controller = makeController({ hasDirtyOverride: false });
      render(<FormViewManagerDialog controller={controller} onClose={onClose} />);
      await userEvent.click(screen.getByText("Close"));
      expect(onClose).toHaveBeenCalledTimes(1);
      expect(screen.queryByText("Unsaved view changes")).toBeNull();
    });

    it("prompts with the same three-way confirmation used for view switching when closing with a dirty override", async () => {
      const onClose = vi.fn();
      const controller = makeController({
        hasDirtyOverride: true,
        hiddenCount: 1,
        override: { hiddenFieldIds: new Set(["apparel"]), isDirty: true },
      });
      render(<FormViewManagerDialog controller={controller} onClose={onClose} />);

      await userEvent.click(screen.getByLabelText("Close"));
      expect(onClose).not.toHaveBeenCalled();
      expect(screen.getByText("Unsaved view changes")).toBeTruthy();

      await userEvent.click(screen.getByText("Discard changes and switch"));
      expect(controller.resetOverride).toHaveBeenCalledTimes(1);
      expect(onClose).toHaveBeenCalledTimes(1);
    });

    it("Cancel in the close confirmation keeps the dialog open", async () => {
      const onClose = vi.fn();
      const controller = makeController({
        hasDirtyOverride: true,
        hiddenCount: 1,
        override: { hiddenFieldIds: new Set(["apparel"]), isDirty: true },
      });
      render(<FormViewManagerDialog controller={controller} onClose={onClose} />);

      await userEvent.click(screen.getByText("Close"));
      await userEvent.click(screen.getByText("Cancel"));
      expect(screen.queryByText("Unsaved view changes")).toBeNull();
      expect(screen.getByText("Manage Form Views")).toBeTruthy();
      expect(onClose).not.toHaveBeenCalled();
    });

    it("saving as custom from the close confirmation saves the override and then closes", async () => {
      const onClose = vi.fn();
      const controller = makeController({
        hasDirtyOverride: true,
        hiddenCount: 1,
        override: { hiddenFieldIds: new Set(["apparel"]), isDirty: true },
      });
      render(<FormViewManagerDialog controller={controller} onClose={onClose} />);

      await userEvent.click(screen.getByText("Close"));
      await userEvent.click(screen.getByText("Save as custom view"));
      await userEvent.type(screen.getByLabelText("Custom view name"), "My saved view");
      await userEvent.click(screen.getByText("Save and switch"));

      expect(controller.saveOverrideAsCustomView).toHaveBeenCalledWith("My saved view");
      expect(onClose).toHaveBeenCalledTimes(1);
    });

    it("does not dismiss the close confirmation or call onClose if a real Def switch (rerender with a new controller, dialog stays mounted) happens while saving as custom (finding 3)", async () => {
      // Unlike a plain "swap the mock's return value" test, this rerenders the SAME dialog
      // instance with a brand-new `controller` snapshot mid-flight -- exactly what
      // `XmlFormEditor` does when `selectedDef` changes while `managerOpen` stays `true` (the
      // dialog never unmounts on a Def switch). `getScopeKey` is shared across both controller
      // snapshots because that matches the real `useFormViews` contract: it is ONE stable
      // function (identical across every render of the same `useFormViews` instance) that
      // always reports the LIVE scope, regardless of which render's `controller` object a
      // stale closure happens to call it through.
      const onCloseA = vi.fn();
      let resolveSave!: () => void;
      const savePromise = new Promise<void>((resolve) => {
        resolveSave = resolve;
      });
      const getScopeKey = vi.fn().mockReturnValue("scope-A");
      const controllerA = makeController({
        hasDirtyOverride: true,
        hiddenCount: 1,
        override: { hiddenFieldIds: new Set(["apparel"]), isDirty: true },
        saveOverrideAsCustomView: vi.fn().mockReturnValue(savePromise),
        getScopeKey,
      });
      const { rerender } = render(<FormViewManagerDialog controller={controllerA} onClose={onCloseA} />);

      await userEvent.click(screen.getByText("Close"));
      await userEvent.click(screen.getByText("Save as custom view"));
      await userEvent.type(screen.getByLabelText("Custom view name"), "Def A in progress");
      await userEvent.click(screen.getByText("Save and switch"));
      expect(screen.getByText("Saving…")).toBeTruthy();

      // The pane re-renders this SAME dialog instance for a different Def -- a brand-new
      // `controller` prop, no unmount/remount.
      const onCloseB = vi.fn();
      const controllerB = makeController({ getScopeKey });
      rerender(<FormViewManagerDialog controller={controllerB} onClose={onCloseB} />);
      // The live scope has now genuinely moved on.
      getScopeKey.mockReturnValue("scope-B");

      resolveSave();
      await waitFor(() => expect(getScopeKey).toHaveBeenCalledTimes(2));

      // The override WAS saved as a custom view correctly under its original scope, but this
      // stale completion must never dismiss the close-confirmation dialog or fire either
      // `onClose` -- not `onCloseA`'s (the scope it started under) nor `onCloseB`'s (the scope
      // that's active now, which this stale save has nothing to do with).
      expect(onCloseA).not.toHaveBeenCalled();
      expect(onCloseB).not.toHaveBeenCalled();
      expect(screen.getByText("Unsaved view changes")).toBeTruthy();
      // `busy` must still have reset (FormViewSwitchConfirmDialog's own `finally`) even though
      // the dialog was deliberately not dismissed -- otherwise Save/Back would be stuck disabled.
      expect(screen.getByText("Save and switch")).toBeTruthy();
    });
  });

  describe("scope-staleness guards on local dialog state (finding 3, round 2)", () => {
    it("does not clear the New custom view name input or auto-select if a real Def switch happens while Create is in flight", async () => {
      let resolveCreate!: (value: unknown) => void;
      const createPromise = new Promise((resolve) => {
        resolveCreate = resolve;
      });
      const getScopeKey = vi.fn().mockReturnValue("scope-A");
      const controllerA = makeController({
        getScopeKey,
        createCustomView: vi.fn().mockReturnValue(createPromise),
      });
      const { rerender } = render(<FormViewManagerDialog controller={controllerA} onClose={vi.fn()} />);

      await userEvent.type(screen.getByLabelText("New custom view name"), "Def A view");
      await userEvent.click(screen.getByText("Create"));
      expect(screen.getByText("Creating…")).toBeTruthy();

      // Same dialog instance, re-rendered for a different Def (no unmount).
      const controllerB = makeController({ getScopeKey });
      rerender(<FormViewManagerDialog controller={controllerB} onClose={vi.fn()} />);

      // The user starts a brand-new, unrelated create attempt for the CURRENT (new) scope.
      const input = screen.getByLabelText("New custom view name") as HTMLInputElement;
      await userEvent.clear(input);
      await userEvent.type(input, "Def B view");

      getScopeKey.mockReturnValue("scope-B");
      resolveCreate({
        id: "created-under-scope-a",
        target: { gameVersion: "1.6", defType: "ThingDef" },
        name: "Def A view",
        description: null,
        hiddenFieldIds: [],
        baseSchemaView: null,
        createdAt: "2026-01-01T00:00:00Z",
        updatedAt: "2026-01-01T00:00:00Z",
      });
      await waitFor(() => expect(getScopeKey).toHaveBeenCalledTimes(2));

      // The stale Create must not clobber what the user is now typing for scope B, and must
      // not auto-select the scope-A view under controllerA's stale `selectView`.
      expect((screen.getByLabelText("New custom view name") as HTMLInputElement).value).toBe(
        "Def B view",
      );
      expect(controllerA.selectView).not.toHaveBeenCalled();
      // The busy flag must not be left stuck -- Create is a local-only concern, safe to reset
      // regardless of scope.
      expect(screen.queryByText("Creating…")).toBeNull();
    });

    it("does not surface a Duplicate failure's error banner if the scope changes before it rejects", async () => {
      let rejectDuplicate!: (e: unknown) => void;
      const duplicatePromise = new Promise((_resolve, reject) => {
        rejectDuplicate = reject;
      });
      const getScopeKey = vi.fn().mockReturnValueOnce("scope-A").mockReturnValue("scope-B");
      const weapon = { id: "weapon", origin: "schema" as const };
      const controller = makeController({
        availableViews: [
          {
            id: "default",
            targetDefType: "ThingDef",
            label: "Default View",
            order: 0,
            origin: "default",
            hiddenFieldIds: [],
            recommended: false,
          },
          {
            id: weapon.id,
            targetDefType: "ThingDef",
            label: "Weapon",
            order: 10,
            origin: weapon.origin,
            hiddenFieldIds: [],
            recommended: false,
          },
        ],
        duplicateAsCustomView: vi.fn().mockReturnValue(duplicatePromise),
        getScopeKey,
      });
      render(<FormViewManagerDialog controller={controller} onClose={vi.fn()} />);

      const duplicateButtons = screen.getAllByText("Duplicate");
      await userEvent.click(duplicateButtons[1]); // the schema row's Duplicate

      rejectDuplicate(new Error("network error"));
      await waitFor(() => expect(getScopeKey).toHaveBeenCalledTimes(2));

      // A stale failure from an abandoned scope must never surface as an error banner against
      // whatever Def/scope is actually being looked at now.
      expect(screen.queryByText("network error")).toBeNull();
    });

    it("does not clear a DIFFERENT in-progress rename if a stale rename completion resolves after the scope changed", async () => {
      let resolveRename!: (value: unknown) => void;
      const renamePromise = new Promise((resolve) => {
        resolveRename = resolve;
      });
      const custom = {
        id: "custom-1",
        targetDefType: "ThingDef",
        label: "My view",
        order: 0,
        origin: "custom" as const,
        hiddenFieldIds: [],
        recommended: false,
      };
      const getScopeKey = vi.fn().mockReturnValueOnce("scope-A").mockReturnValue("scope-B");
      const controller = makeController({
        availableViews: [
          {
            id: "default",
            targetDefType: "ThingDef",
            label: "Default View",
            order: 0,
            origin: "default",
            hiddenFieldIds: [],
            recommended: false,
          },
          custom,
        ],
        renameCustomView: vi.fn().mockReturnValue(renamePromise),
        getScopeKey,
      });
      render(<FormViewManagerDialog controller={controller} onClose={vi.fn()} />);

      await userEvent.click(screen.getByLabelText("Rename My view"));
      const input = screen.getByLabelText("Rename My view") as HTMLInputElement;
      await userEvent.clear(input);
      await userEvent.type(input, "Renamed under scope A");
      await userEvent.click(screen.getByLabelText("Confirm rename"));

      // The scope moves on before this rename resolves (e.g. the user switched Defs in this
      // same pane). `renameCustomView` still lands correctly under its original scope, but
      // `commitRename` must not exit rename mode (`setRenamingId(null)`) here -- doing so would
      // silently close out of whatever rename state now belongs to the CURRENT scope.
      resolveRename({ ...custom, name: "Renamed under scope A" });
      await waitFor(() => expect(getScopeKey).toHaveBeenCalledTimes(2));

      // The rename input must still be showing/editable (not silently exited) since the stale
      // completion's scope no longer matches.
      expect(screen.getByLabelText("Rename My view")).toBeTruthy();
    });
  });
});
