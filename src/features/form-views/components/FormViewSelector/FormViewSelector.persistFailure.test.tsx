import { screen, waitFor } from "@testing-library/react";
import { renderWithI18n as render } from "../../../../i18n/testing/renderWithI18n";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { FormViewSelector } from "./FormViewSelector";
import { useFormViews } from "../../hooks/useFormViews";
import type { SchemaCatalog } from "../../../schema-catalog";

// Real `useFormViews` + real `FormViewSelector` -- only the Tauri `invoke` boundary is mocked --
// so this exercises the actual failure path end to end: a real `set_last_selected_form_view`
// rejection must actually reach the rendered UI, not just live in hook state.
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
        formViews: {
          minimal: {
            id: "minimal",
            label: "Minimal",
            order: 10,
            recommended: false,
            hiddenFieldIds: [],
            declaredOnDefType: "ThingDef",
          },
        },
      },
    },
  };
}

function Harness() {
  const formViews = useFormViews({
    projectId: "proj1",
    gameVersion: "1.6",
    catalog: makeCatalog(),
    pane: null,
    selectedDef: { defType: "ThingDef", ordinal: 0 },
  });
  return <FormViewSelector controller={formViews} onOpenManager={() => undefined} />;
}

describe("FormViewSelector - persisted-selection failure surfaces visibly (real useFormViews)", () => {
  it("renders a visible warning when set_last_selected_form_view actually rejects", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "list_custom_form_views") return Promise.resolve({ views: [], warning: null });
      if (cmd === "get_last_selected_form_view") return Promise.resolve({ selected: null, warning: null });
      if (cmd === "set_last_selected_form_view") return Promise.reject(new Error("disk full"));
      return Promise.reject(new Error(`unexpected invoke: ${cmd}`));
    });

    render(<Harness />);

    expect(screen.queryByRole("alert")).toBeNull();

    await userEvent.selectOptions(await screen.findByLabelText("View"), "schema:minimal");

    // The selection still takes effect in-memory (the whole point of the warning is that it
    // silently would NOT have persisted otherwise) ...
    await waitFor(() =>
      expect((screen.getByLabelText("View") as HTMLSelectElement).value).toBe("schema:minimal"),
    );
    // ... but the failed persist must be visible in the UI, not just in hook state.
    await waitFor(() =>
      expect(screen.getByRole("alert").textContent).toBe(
        "Could not save your Form View selection.",
      ),
    );
  });

  it("clears the warning once a later persist succeeds", async () => {
    let shouldFail = true;
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "list_custom_form_views") return Promise.resolve({ views: [], warning: null });
      if (cmd === "get_last_selected_form_view") return Promise.resolve({ selected: null, warning: null });
      if (cmd === "set_last_selected_form_view") {
        return shouldFail ? Promise.reject(new Error("disk full")) : Promise.resolve(undefined);
      }
      return Promise.reject(new Error(`unexpected invoke: ${cmd}`));
    });

    render(<Harness />);

    await userEvent.selectOptions(await screen.findByLabelText("View"), "schema:minimal");
    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeNull());

    shouldFail = false;
    await userEvent.selectOptions(screen.getByLabelText("View"), "default:default");
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
  });
});
