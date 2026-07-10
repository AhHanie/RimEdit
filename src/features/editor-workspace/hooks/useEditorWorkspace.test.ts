import { renderHook, act, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { useEditorWorkspace } from "./useEditorWorkspace";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

function emptyScan(projectId: string) {
  return { projectId, projectRoot: "/tmp/proj", folders: [], files: [] };
}

describe("useEditorWorkspace openTab editorKindHint", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("opens a tab as XML using the hint even when the file isn't in the scan yet", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "scan_project_files") return Promise.resolve(emptyScan("proj1"));
      throw new Error(`unexpected invoke: ${cmd}`);
    });

    const { result } = renderHook(() => useEditorWorkspace("proj1"));
    await waitFor(() => expect(result.current.scan).not.toBeNull());

    // Simulate opening a tab for a file just created via createFile, before a
    // fresh scan has landed -- scan.files is still empty here.
    act(() => {
      result.current.openTab({
        locationId: "proj1",
        sourceKind: "project",
        readOnly: false,
        relativePath: "Patches/NewPatches.xml",
        editorKindHint: "xml",
      });
    });

    expect(result.current.tabs).toHaveLength(1);
    expect(result.current.tabs[0].editorKind).toBe("xml");
  });

  it("falls back to the scan-derived kind when no hint is given", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "scan_project_files") return Promise.resolve(emptyScan("proj1"));
      throw new Error(`unexpected invoke: ${cmd}`);
    });

    const { result } = renderHook(() => useEditorWorkspace("proj1"));
    await waitFor(() => expect(result.current.scan).not.toBeNull());

    act(() => {
      result.current.openTab({
        locationId: "proj1",
        sourceKind: "project",
        readOnly: false,
        relativePath: "Patches/Unknown.xml",
      });
    });

    expect(result.current.tabs[0].editorKind).toBe("unsupported");
  });
});
