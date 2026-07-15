import { renderHook, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { useProjectFiles } from "./useProjectFiles";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

describe("useProjectFiles", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("rejects mutating calls with a structured diagnostic code, not just a raw English message", async () => {
    // The "no active project" precondition guarding every mutating call (create/rename/delete a
    // file or folder) is a known condition this hook itself detects -- it must carry a `code` the
    // shared renderer can translate (see `src/i18n/diagnostics.ts`), not only an English
    // `Error.message` that bypasses localization entirely. Mirrors
    // `useCustomFormViews.test.ts`'s equivalent assertion for the same bug class.
    const { result } = renderHook(() => useProjectFiles(undefined));
    await waitFor(() => expect(result.current.loadingScan).toBe(false));

    await expect(result.current.createFile("", "a.xml")).rejects.toMatchObject({
      code: "project_file_no_active_project",
    });
    await expect(result.current.createFolder("", "Sub")).rejects.toMatchObject({
      code: "project_file_no_active_project",
    });
    await expect(result.current.renamePath("a.xml", "b.xml", "file")).rejects.toMatchObject({
      code: "project_file_no_active_project",
    });
    await expect(result.current.deletePath("a.xml", "file")).rejects.toMatchObject({
      code: "project_file_no_active_project",
    });

    expect(invokeMock).not.toHaveBeenCalled();
  });
});
