import { screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { renderWithI18n as render } from "../../../../i18n/testing/renderWithI18n";
import { IndexErrorsDialog } from "./IndexErrorsDialog";
import type { DefIndexErrorsResponse } from "../../types";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

const oneError: DefIndexErrorsResponse = {
  errors: [
    {
      locationId: "src1",
      locationName: "My Workshop Mod",
      sourceKind: "source",
      relativePath: "Defs/ThingDefs/Broken.xml",
      code: "def_index_parse_error",
      message: "Unexpected token",
      line: 12,
      column: 3,
    },
  ],
  total: 1,
  truncated: false,
};

beforeEach(() => {
  invokeMock.mockReset();
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("IndexErrorsDialog", () => {
  it("shows a loading state before the request resolves", () => {
    invokeMock.mockImplementation(() => new Promise(() => {}));
    render(<IndexErrorsDialog projectId="proj1" onClose={() => {}} />);
    expect(screen.getByText("Loading errors…")).toBeDefined();
  });

  it("fetches errors for the given project and renders location, path, and a catalog-rendered message", async () => {
    invokeMock.mockImplementation((cmd, args) => {
      if (cmd === "get_def_index_errors") {
        expect(args).toEqual({ projectId: "proj1" });
        return Promise.resolve(oneError);
      }
      return Promise.reject(new Error(`unexpected command ${cmd}`));
    });
    render(<IndexErrorsDialog projectId="proj1" onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByText("My Workshop Mod")).toBeDefined();
    });
    expect(screen.getByText(/Defs\/ThingDefs\/Broken\.xml/)).toBeDefined();
    // `def_index_parse_error` has a catalog entry (src/i18n/resources/en/diagnostics.json), so
    // the dialog renders it through the shared `renderDiagnostic` pipeline rather than the raw
    // backend `message` -- proving the catalog entries added for this feature are actually wired
    // up, not dead weight.
    expect(
      screen.getByText("This file has a parse error and was skipped while building the Def index."),
    ).toBeDefined();
    expect(screen.queryByText("Unexpected token")).toBeNull();
    expect(screen.getByText("def_index_parse_error")).toBeDefined();
  });

  it("falls back to the raw message for a code with no catalog entry", async () => {
    invokeMock.mockImplementation(() =>
      Promise.resolve({
        errors: [
          {
            locationId: "src1",
            locationName: "My Mod",
            sourceKind: "source",
            relativePath: "Defs/a.xml",
            code: "some_future_backend_only_code",
            message: "A backend-specific technical detail with no catalog entry yet.",
          },
        ],
        total: 1,
        truncated: false,
      } satisfies DefIndexErrorsResponse),
    );
    render(<IndexErrorsDialog projectId="proj1" onClose={() => {}} />);

    await waitFor(() => {
      expect(
        screen.getByText("A backend-specific technical detail with no catalog entry yet."),
      ).toBeDefined();
    });
  });

  it("interpolates typed args into the catalog string", async () => {
    invokeMock.mockImplementation(() =>
      Promise.resolve({
        errors: [
          {
            locationId: "src1",
            locationName: "My Workshop Item",
            sourceKind: "source",
            relativePath: "222/LoadFolders.xml",
            code: "load_folder_missing",
            message: "LoadFolders.xml references missing folder: 1.6/Compat",
            args: { folder: "1.6/Compat" },
          },
        ],
        total: 1,
        truncated: false,
      } satisfies DefIndexErrorsResponse),
    );
    render(<IndexErrorsDialog projectId="proj1" onClose={() => {}} />);

    await waitFor(() => {
      expect(
        screen.getByText(
          'This item\'s LoadFolders.xml references a missing folder: 1.6/Compat.',
        ),
      ).toBeDefined();
    });
  });

  it("shows a location scan fallback when relativePath is absent", async () => {
    invokeMock.mockImplementation(() =>
      Promise.resolve({
        errors: [
          {
            locationId: "src1",
            locationName: "Broken Collection",
            sourceKind: "source",
            code: "def_index_location_scan_failed",
            message: "Could not read location",
          },
        ],
        total: 1,
        truncated: false,
      } satisfies DefIndexErrorsResponse),
    );
    render(<IndexErrorsDialog projectId="proj1" onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByText("Location scan")).toBeDefined();
    });
  });

  it("shows an empty state when there are no errors", async () => {
    invokeMock.mockImplementation(() =>
      Promise.resolve({ errors: [], total: 0, truncated: false } satisfies DefIndexErrorsResponse),
    );
    render(<IndexErrorsDialog projectId="proj1" onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByText("No index errors")).toBeDefined();
    });
  });

  it("shows a truncation notice when the response is capped", async () => {
    invokeMock.mockImplementation(() =>
      Promise.resolve({
        errors: [oneError.errors[0]],
        total: 250,
        truncated: true,
      } satisfies DefIndexErrorsResponse),
    );
    render(<IndexErrorsDialog projectId="proj1" onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByText("Showing 1 of 250 errors")).toBeDefined();
    });
  });

  it("shows a request-failure state without silently disappearing", async () => {
    invokeMock.mockImplementation(() => Promise.reject(new Error("boom")));
    render(<IndexErrorsDialog projectId="proj1" onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByText("Failed to load index errors")).toBeDefined();
    });
  });

  it("copies a redacted text report without exposing the location root", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });
    invokeMock.mockImplementation(() => Promise.resolve(oneError));
    const { fireEvent } = await import("@testing-library/react");
    render(<IndexErrorsDialog projectId="proj1" onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByText("My Workshop Mod")).toBeDefined();
    });
    fireEvent.click(screen.getByText("Copy details"));

    await waitFor(() => {
      expect(writeText).toHaveBeenCalledTimes(1);
    });
    const copiedText = writeText.mock.calls[0][0] as string;
    expect(copiedText).toContain("My Workshop Mod");
    expect(copiedText).toContain("Defs/ThingDefs/Broken.xml");
    expect(copiedText).not.toContain("C:\\");
  });
});
