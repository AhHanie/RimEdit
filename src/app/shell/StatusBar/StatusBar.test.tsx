import { screen } from "@testing-library/react";
import { renderWithI18n } from "../../../i18n/testing/renderWithI18n";
import { StatusBar } from "./StatusBar";

function defaultProps(overrides: Partial<Parameters<typeof StatusBar>[0]> = {}) {
  return {
    hasActiveProject: true,
    loadingScan: false,
    scanError: null,
    fileCount: 0,
    activeFilePath: null,
    activeFileSizeBytes: null,
    themeMode: "system" as const,
    indexingStatus: null,
    ...overrides,
  };
}

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
});
