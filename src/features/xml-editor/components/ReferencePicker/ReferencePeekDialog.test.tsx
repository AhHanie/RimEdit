import { screen } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { renderWithI18n as render } from "../../../../i18n/testing/renderWithI18n";
import { ReferencePeekDialog } from "./ReferencePeekDialog";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

describe("ReferencePeekDialog", () => {
  it("renders a load failure's code/args through the shared diagnostic catalog, not a stringified error object", async () => {
    invokeMock.mockRejectedValue({
      // Same wire shape a rejected Tauri command carries (`AppError`'s `code`/`args`).
      // `String(e)` on a plain object like this would render `[object Object]` -- proving the
      // dialog instead renders the translated code/args lookup guards against that regression.
      code: "invalid_location_path",
      message: "backend raw message that must not be shown",
      args: { path: "Defs/Thing.xml" },
    });

    render(
      <ReferencePeekDialog
        projectId="proj1"
        locationId="loc1"
        relativePath="Defs/Thing.xml"
        defName="Wall"
        defType="ThingDef"
        onClose={vi.fn()}
      />,
    );

    expect(
      await screen.findByText('"Defs/Thing.xml" is not a valid location path.'),
    ).toBeTruthy();
    expect(screen.queryByText("[object Object]")).toBeFalsy();
    expect(
      screen.queryByText("backend raw message that must not be shown"),
    ).toBeFalsy();
  });
});
