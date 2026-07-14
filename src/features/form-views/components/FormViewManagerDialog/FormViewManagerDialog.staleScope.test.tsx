import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { FormViewManagerDialog } from "./FormViewManagerDialog";
import { useFormViews } from "../../hooks/useFormViews";
import type { SchemaCatalog } from "../../../schema-catalog";

// Real `useFormViews` + real `FormViewManagerDialog` -- only the Tauri `invoke` boundary is
// mocked -- so this exercises the actual stale-scope path end to end: closing the dialog and
// switching to a different Def/scope while a Create is still in flight must not apply/persist/
// warn against the newly active scope once that Create resolves.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

function makeCatalog(): SchemaCatalog {
  return {
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
            required: true,
            examples: [],
            repeatable: false,
            xml: "element",
            flags: false,
          },
        },
        // Recommended, so both scope A and scope B resolve here by default (no persisted
        // preference in this test) -- a stable, observable baseline distinct from both Default
        // and the view created under scope A.
        formViews: {
          weapon: {
            id: "weapon",
            label: "Weapon",
            order: 10,
            recommended: true,
            hiddenFieldIds: [],
            declaredOnDefType: "ThingDef",
          },
        },
      },
    },
  };
}

function Harness({ ordinal, dialogOpen }: { ordinal: number; dialogOpen: boolean }) {
  const formViews = useFormViews({
    projectId: "proj1",
    gameVersion: "1.6",
    catalog: makeCatalog(),
    pane: null,
    selectedDef: { defType: "ThingDef", ordinal },
  });
  return (
    <div>
      <div data-testid="selected">{`${formViews.selectedView.origin}:${formViews.selectedView.id}`}</div>
      <div data-testid="persist-warning">{formViews.persistWarning ?? ""}</div>
      {dialogOpen && <FormViewManagerDialog controller={formViews} onClose={() => undefined} />}
    </div>
  );
}

describe("FormViewManagerDialog - stale scope after Create resolves (real useFormViews)", () => {
  it("does not apply, persist, or warn against a different Def opened while Create was in flight", async () => {
    let resolveCreate!: (view: unknown) => void;
    const createPromise = new Promise((resolve) => {
      resolveCreate = resolve;
    });
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "list_custom_form_views") return Promise.resolve({ views: [], warning: null });
      if (cmd === "get_last_selected_form_view")
        return Promise.resolve({ selected: null, warning: null });
      if (cmd === "create_custom_form_view") return createPromise;
      // A stale chained `selectView` firing would call this -- asserted against below via
      // `invokeMock` call history, but also allow it to resolve harmlessly if it does fire so
      // the test can observe the resulting (wrong) state rather than hanging.
      if (cmd === "set_last_selected_form_view") return Promise.resolve(undefined);
      return Promise.reject(new Error(`unexpected invoke: ${cmd}`));
    });

    const { rerender } = render(<Harness ordinal={0} dialogOpen={true} />);
    await waitFor(() => expect(screen.getByTestId("selected").textContent).toBe("schema:weapon"));

    // Start Create in the manager dialog for ordinal 0 (Def A). Deliberately not resolved yet.
    await userEvent.type(screen.getByLabelText("New custom view name"), "My saved view");
    await userEvent.click(screen.getByText("Create"));

    // Simulate "close the dialog and switch to a different Def" while the create is pending.
    rerender(<Harness ordinal={1} dialogOpen={false} />);
    await waitFor(() => expect(screen.getByTestId("selected").textContent).toBe("schema:weapon"));

    // Now the stale create resolves.
    resolveCreate({
      id: "created-under-def-a",
      target: { gameVersion: "1.6", defType: "ThingDef" },
      name: "My saved view",
      description: null,
      hiddenFieldIds: [],
      baseSchemaView: null,
      createdAt: "2026-01-01T00:00:00Z",
      updatedAt: "2026-01-01T00:00:00Z",
    });
    // Give any (incorrect) chained selectView/persist call every chance to fire and apply.
    await new Promise((resolve) => setTimeout(resolve, 0));
    await new Promise((resolve) => setTimeout(resolve, 0));

    // Def B's (ordinal 1) selection must remain its own recommended view -- not switched to the
    // view created for Def A, and no persist warning displayed against it.
    expect(screen.getByTestId("selected").textContent).toBe("schema:weapon");
    expect(screen.getByTestId("persist-warning").textContent).toBe("");
    // The stale create must never have chained a persist call at all.
    expect(invokeMock).not.toHaveBeenCalledWith(
      "set_last_selected_form_view",
      expect.objectContaining({ id: "created-under-def-a" }),
    );
  });
});
