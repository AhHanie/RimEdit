import { renderHook, act } from "@testing-library/react";
import { vi, describe, it, expect, beforeEach, afterEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { useXmlEditorSession } from "./useXmlEditorSession";
import type { XmlEditorFileRef } from "./useXmlEditorSession";
import type {
  ValidationDiagnostic,
  XmlEditorDocumentLoadResult,
} from "../types/xmlDocument";
import type { SavePreview } from "../types/projectSave";

// An open XML editor session must revalidate against a rebuilt def index
// once `AppShell`'s `validationRefreshRevision` bumps (a completed pending/running -> complete
// transition), without discarding unsaved edits, dirty state, or undo/redo history -- and it must
// not do so through the debounced-typing `parse_xml_editor_buffer` path (which only fires on an
// actual keystroke), so these tests use fake timers and never advance the 300ms typing debounce.

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

function fileRef(overrides: Partial<XmlEditorFileRef> = {}): XmlEditorFileRef {
  return {
    locationId: "loc1",
    sourceKind: "project",
    readOnly: false,
    relativePath: "Defs/A.xml",
    ...overrides,
  };
}

function marker(code: string): ValidationDiagnostic {
  return {
    relativePath: "Defs/A.xml",
    nodeId: null,
    line: null,
    column: null,
    severity: "Warning",
    message: code,
    code,
    defType: null,
    defName: null,
    fieldPath: null,
    blocking: false,
  };
}

function loadResult(
  rawXml: string,
  validationDiagnostics: ValidationDiagnostic[] = [],
): XmlEditorDocumentLoadResult {
  return {
    projectId: "proj1",
    relativePath: "Defs/A.xml",
    rawXml,
    document: {
      nodeCount: 1,
      rootElement: "Defs",
      profile: "defs",
      about: null,
      defs: [],
    },
    parseDiagnostics: [],
    validationDiagnostics,
  };
}

describe("useXmlEditorSession - revalidation after a completed index rebuild", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it("revalidates a dirty buffer without discarding the edit, dirty flag, or history", async () => {
    const initialXml = "<Defs><ThingDef><defName>Steel</defName></ThingDef></Defs>";
    const editedXml = "<Defs><ThingDef><defName>SteelEdited</defName></ThingDef></Defs>";

    invokeMock.mockImplementation((cmd) => {
      if (cmd === "read_project_xml_editor_document") {
        return Promise.resolve(loadResult(initialXml, [marker("stale_diagnostic")]));
      }
      return Promise.reject(new Error(`unexpected command ${String(cmd)}`));
    });

    const { result, rerender } = renderHook(
      ({ revision }: { revision: number }) =>
        useXmlEditorSession("proj1", fileRef(), revision),
      { initialProps: { revision: 0 } },
    );

    // Flush the initial load's microtasks without advancing any timer.
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current?.loading).toBe(false);

    act(() => {
      result.current!.updateRawXml(editedXml);
    });
    expect(result.current!.dirty).toBe(true);
    expect(result.current!.currentRawXml).toBe(editedXml);

    let resolveRevalidate!: (v: XmlEditorDocumentLoadResult) => void;
    invokeMock.mockImplementation((cmd, args) => {
      if (cmd === "parse_xml_editor_buffer") {
        expect(args).toMatchObject({
          projectId: "proj1",
          relativePath: "Defs/A.xml",
          rawXml: editedXml,
        });
        return new Promise((res) => {
          resolveRevalidate = res;
        });
      }
      return Promise.reject(new Error(`unexpected command ${String(cmd)}`));
    });

    // The revision bump alone must trigger exactly this call -- the 300ms typing-debounce timer
    // is never advanced in this test.
    rerender({ revision: 1 });
    expect(invokeMock).toHaveBeenCalledWith(
      "parse_xml_editor_buffer",
      expect.objectContaining({ rawXml: editedXml }),
    );

    await act(async () => {
      resolveRevalidate(loadResult(editedXml, [marker("revalidated_marker")]));
    });

    expect(result.current!.dirty).toBe(true);
    expect(result.current!.baseRawXml).toBe(initialXml);
    expect(result.current!.currentRawXml).toBe(editedXml);
    expect(result.current!.currentValidationDiagnostics).toEqual([
      marker("revalidated_marker"),
    ]);
    expect(result.current!.isBufferValid).toBe(true);
  });

  it("does not fire while a save is in flight, and retries once it clears", async () => {
    const xml = "<Defs><ThingDef><defName>Steel</defName></ThingDef></Defs>";

    // Both command handlers must be registered up front (not swapped in later): the retry this
    // test proves out happens synchronously within the same `act` flush that resolves the
    // initial load, before there is any chance to re-point the mock afterward.
    let resolveInitialLoad!: (v: XmlEditorDocumentLoadResult) => void;
    let sawRevalidateCall = false;
    invokeMock.mockImplementation((cmd) => {
      if (cmd === "read_project_xml_editor_document") {
        return new Promise((res) => {
          resolveInitialLoad = res;
        });
      }
      if (cmd === "parse_xml_editor_buffer") {
        sawRevalidateCall = true;
        return new Promise(() => {
          // never resolves -- only presence of the call matters here
        });
      }
      return Promise.reject(new Error(`unexpected command ${String(cmd)}`));
    });

    const { result, rerender } = renderHook(
      ({ revision }: { revision: number }) =>
        useXmlEditorSession("proj1", fileRef(), revision),
      { initialProps: { revision: 0 } },
    );

    // Bump the revision while the initial load is still in flight (`loading: true`).
    rerender({ revision: 1 });
    expect(sawRevalidateCall).toBe(false);

    // `loading` clearing is itself an effect dependency, so the same revision value (never
    // consumed while busy above) must now be retried without needing another bump.
    await act(async () => {
      resolveInitialLoad(loadResult(xml));
    });
    expect(result.current?.loading).toBe(false);

    expect(sawRevalidateCall).toBe(true);
  });

  it("does not fire while a save is in flight (saveBusy), and retries once it clears", async () => {
    // Regression for a test-coverage gap: the sibling test above ("does not fire while a save is
    // in flight...") only ever exercises the `loading` half of the `if (loading || saveBusy)
    // return;` guard (useXmlEditorSession.ts). This test specifically drives `saveBusy` via
    // `requestSavePreview`, so a future edit that dropped `saveBusy` from that guard's condition
    // or its effect's dependency array would actually be caught here.
    const xml = "<Defs><ThingDef><defName>Steel</defName></ThingDef></Defs>";

    let resolveSavePreview!: (v: SavePreview) => void;
    let sawRevalidateCall = false;
    invokeMock.mockImplementation((cmd) => {
      if (cmd === "read_project_xml_editor_document") {
        return Promise.resolve(loadResult(xml));
      }
      if (cmd === "preview_project_xml_save") {
        return new Promise((res) => {
          resolveSavePreview = res;
        });
      }
      if (cmd === "parse_xml_editor_buffer") {
        sawRevalidateCall = true;
        return new Promise(() => {
          // never resolves -- only presence of the call matters here
        });
      }
      return Promise.reject(new Error(`unexpected command ${String(cmd)}`));
    });

    const { result, rerender } = renderHook(
      ({ revision }: { revision: number }) =>
        useXmlEditorSession("proj1", fileRef(), revision),
      { initialProps: { revision: 0 } },
    );
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current?.loading).toBe(false);

    // Kick off a save preview -- `saveBusy` becomes true and stays true until it resolves.
    act(() => {
      void result.current!.requestSavePreview();
    });
    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current?.saveBusy).toBe(true);

    // Bump the revision while the save is still in flight.
    rerender({ revision: 1 });
    expect(sawRevalidateCall).toBe(false);

    // Resolving the save preview clears `saveBusy`, which is itself an effect dependency, so the
    // same revision value (never consumed while busy above) must now be retried without needing
    // another bump.
    await act(async () => {
      resolveSavePreview({
        projectId: "proj1",
        relativePath: "Defs/A.xml",
        currentHash: "a",
        proposedHash: "b",
        changed: true,
        diff: [],
        validationToken: "tok",
      });
    });
    expect(result.current?.saveBusy).toBe(false);

    expect(sawRevalidateCall).toBe(true);
  });

  it("syncs displayed text for a read-only source tab so diagnostics never describe stale content", async () => {
    const oldXml = "<Defs><ThingDef><defName>Steel</defName></ThingDef></Defs>";
    // The file changed on disk (e.g. edited externally, or the source mod updated) between the
    // initial load and the revalidation triggered by a completed rebuild.
    const newXml = "<Defs><ThingDef><defName>SteelRenamed</defName></ThingDef></Defs>";

    invokeMock.mockImplementation((cmd) => {
      if (cmd === "read_location_xml_editor_document") {
        return Promise.resolve(loadResult(oldXml));
      }
      return Promise.reject(new Error(`unexpected command ${String(cmd)}`));
    });

    const { result, rerender } = renderHook(
      ({ revision }: { revision: number }) =>
        useXmlEditorSession("proj1", fileRef({ readOnly: true }), revision),
      { initialProps: { revision: 0 } },
    );

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current?.loading).toBe(false);
    expect(result.current?.currentRawXml).toBe(oldXml);

    invokeMock.mockImplementation((cmd, args) => {
      if (cmd === "read_location_xml_editor_document") {
        expect(args).toMatchObject({
          projectId: "proj1",
          locationId: "loc1",
          relativePath: "Defs/A.xml",
        });
        return Promise.resolve(loadResult(newXml, [marker("revalidated_marker")]));
      }
      return Promise.reject(new Error(`unexpected command ${String(cmd)}`));
    });

    rerender({ revision: 1 });

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    // The displayed text must be updated to match the diagnostics that were just applied --
    // otherwise the diagnostics would describe content the user isn't looking at.
    expect(result.current?.currentRawXml).toBe(newXml);
    expect(result.current?.currentValidationDiagnostics).toEqual([
      marker("revalidated_marker"),
    ]);
    // Read-only tabs are never "dirty" regardless of what the displayed text is.
    expect(result.current?.dirty).toBe(false);
  });

  it("discards a stale revalidation result once the buffer has since changed, without reverting text or diagnostics", async () => {
    const oldXml = "<Defs><ThingDef><defName>Steel</defName></ThingDef></Defs>";
    const editedXml = "<Defs><ThingDef><defName>SteelEdited</defName></ThingDef></Defs>";

    invokeMock.mockImplementation((cmd) => {
      if (cmd === "read_project_xml_editor_document") {
        return Promise.resolve(loadResult(oldXml));
      }
      return Promise.reject(new Error(`unexpected command ${String(cmd)}`));
    });

    const { result, rerender } = renderHook(
      ({ revision }: { revision: number }) =>
        useXmlEditorSession("proj1", fileRef(), revision),
      { initialProps: { revision: 0 } },
    );
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current?.loading).toBe(false);

    let resolveRevalidate!: (v: XmlEditorDocumentLoadResult) => void;
    invokeMock.mockImplementation((cmd) => {
      if (cmd === "parse_xml_editor_buffer") {
        return new Promise((res) => {
          resolveRevalidate = res;
        });
      }
      return Promise.reject(new Error(`unexpected command ${String(cmd)}`));
    });

    rerender({ revision: 1 });

    // The buffer changes while the revalidation triggered above is still in flight.
    act(() => {
      result.current!.updateRawXml(editedXml);
    });
    expect(result.current!.currentRawXml).toBe(editedXml);

    // The stale response -- describing the buffer as it was *before* the edit -- resolves after.
    await act(async () => {
      resolveRevalidate(loadResult(oldXml, [marker("stale_marker")]));
    });

    // The stale result must not revert the newer edit's text, nor inject diagnostics that
    // describe content the user no longer has on screen.
    expect(result.current!.currentRawXml).toBe(editedXml);
    expect(result.current!.currentValidationDiagnostics).not.toEqual([
      marker("stale_marker"),
    ]);
  });
});
