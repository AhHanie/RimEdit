import { screen, fireEvent, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { renderWithI18n } from "../../../i18n/testing/renderWithI18n";
import { StatusBar } from "./StatusBar";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

function defaultProps(overrides: Partial<Parameters<typeof StatusBar>[0]> = {}) {
  return {
    hasActiveProject: true,
    loadingScan: false,
    scanError: null,
    fileCount: 0,
    activeFilePath: null,
    activeFileSizeBytes: null,
    indexingStatus: null,
    ...overrides,
  };
}

afterEach(() => {
  vi.clearAllMocks();
});

describe("StatusBar", () => {
  it("shows singular file count", () => {
    renderWithI18n(<StatusBar {...defaultProps({ fileCount: 1 })} />);
    expect(screen.getByText("1 file")).toBeDefined();
  });

  it("shows plural file count", () => {
    renderWithI18n(<StatusBar {...defaultProps({ fileCount: 3 })} />);
    expect(screen.getByText("3 files")).toBeDefined();
  });

  it("shows singular index error count", () => {
    renderWithI18n(
      <StatusBar
        {...defaultProps({
          indexingStatus: {
            phase: "complete",
            pendingFiles: 0,
            indexedDefs: 5,
            projectDefs: 5,
            sourceDefs: 0,
            errors: 1,
            updatedAtUnixMs: 0,
          },
        })}
      />,
    );
    expect(screen.getByText("1 index error")).toBeDefined();
  });

  it("shows plural index error count", () => {
    renderWithI18n(
      <StatusBar
        {...defaultProps({
          indexingStatus: {
            phase: "complete",
            pendingFiles: 0,
            indexedDefs: 5,
            projectDefs: 5,
            sourceDefs: 0,
            errors: 2,
            updatedAtUnixMs: 0,
          },
        })}
      />,
    );
    expect(screen.getByText("2 index errors")).toBeDefined();
  });

  it("shows a discovering label during the discovering stage", () => {
    renderWithI18n(
      <StatusBar
        {...defaultProps({
          indexingStatus: {
            phase: "running",
            pendingFiles: 0,
            indexedDefs: 0,
            projectDefs: 0,
            sourceDefs: 0,
            errors: 0,
            updatedAtUnixMs: 0,
            currentStage: "discovering",
          },
        })}
      />,
    );
    expect(screen.getByText("Discovering files…")).toBeDefined();
  });

  it("shows formatted processed/total file progress during the indexing stage", () => {
    renderWithI18n(
      <StatusBar
        {...defaultProps({
          indexingStatus: {
            phase: "running",
            pendingFiles: 0,
            indexedDefs: 0,
            projectDefs: 0,
            sourceDefs: 0,
            errors: 0,
            updatedAtUnixMs: 0,
            currentStage: "indexing",
            totalFiles: 2840,
            processedFiles: 415,
          },
        })}
      />,
    );
    expect(screen.getByText("Indexing 415 / 2,840 files…")).toBeDefined();
  });

  it("falls back to the generic indexing label when running with no stage (incremental jobs)", () => {
    renderWithI18n(
      <StatusBar
        {...defaultProps({
          indexingStatus: {
            phase: "running",
            pendingFiles: 3,
            indexedDefs: 0,
            projectDefs: 0,
            sourceDefs: 0,
            errors: 0,
            updatedAtUnixMs: 0,
          },
        })}
      />,
    );
    expect(screen.getByText("Indexing…")).toBeDefined();
  });

  it("shows the No project state when there is no active project", () => {
    renderWithI18n(<StatusBar {...defaultProps({ hasActiveProject: false })} />);
    expect(screen.getByText("No project")).toBeDefined();
  });

  it("formats an active file's size through the shared Intl formatter", () => {
    renderWithI18n(
      <StatusBar
        {...defaultProps({ activeFilePath: "Defs/Things.xml", activeFileSizeBytes: 2048 })}
      />,
    );
    expect(screen.getByText("2 kB")).toBeDefined();
  });

  it("renders the index error count as a keyboard-accessible button with a count in its name", () => {
    renderWithI18n(
      <StatusBar
        {...defaultProps({
          projectId: "proj1",
          indexingStatus: {
            phase: "complete",
            pendingFiles: 0,
            indexedDefs: 5,
            projectDefs: 5,
            sourceDefs: 0,
            errors: 2,
            updatedAtUnixMs: 0,
          },
        })}
      />,
    );
    const button = screen.getByRole("button", { name: "2 index errors" });
    expect(button).toBeDefined();
  });

  it("does not render a button for a zero-error complete status", () => {
    renderWithI18n(
      <StatusBar
        {...defaultProps({
          indexingStatus: {
            phase: "complete",
            pendingFiles: 0,
            indexedDefs: 5,
            projectDefs: 5,
            sourceDefs: 0,
            errors: 0,
            updatedAtUnixMs: 0,
          },
        })}
      />,
    );
    expect(screen.queryByRole("button", { name: /index error/ })).toBeNull();
  });

  it("opens the errors dialog on click and fetches errors for the active project", async () => {
    invokeMock.mockImplementation((cmd, args) => {
      if (cmd === "get_def_index_errors") {
        expect(args).toEqual({ projectId: "proj1" });
        return Promise.resolve({ errors: [], total: 0, truncated: false });
      }
      return Promise.reject(new Error(`unexpected command ${cmd}`));
    });
    renderWithI18n(
      <StatusBar
        {...defaultProps({
          projectId: "proj1",
          indexingStatus: {
            phase: "complete",
            pendingFiles: 0,
            indexedDefs: 5,
            projectDefs: 5,
            sourceDefs: 0,
            errors: 1,
            updatedAtUnixMs: 0,
          },
        })}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "1 index error" }));
    expect(screen.getByRole("dialog", { name: "Index Errors" })).toBeDefined();
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("get_def_index_errors", { projectId: "proj1" });
    });
  });
});
