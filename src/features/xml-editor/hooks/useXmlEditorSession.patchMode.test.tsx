import { renderHook, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { useXmlEditorSession } from "./useXmlEditorSession";
import type { XmlEditorFileRef } from "./useXmlEditorSession";
import type { XmlEditorDocumentView } from "../types/xmlDocument";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

function loadResult(document: XmlEditorDocumentView | null, rawXml: string) {
  return {
    projectId: "proj1",
    relativePath: "irrelevant.xml",
    rawXml,
    document,
    parseDiagnostics: [],
    validationDiagnostics: [],
  };
}

function fileRef(relativePath: string): XmlEditorFileRef {
  return { locationId: "proj1", sourceKind: "project", readOnly: false, relativePath };
}

// A <Patch>-rooted file has zero Defs, just like an empty <Defs></Defs> file -- so the mode
// default can't key off `defs.length` alone; it must also recognize the Patch root so opening a
// patch file lands in the (patch tree) form view rather than falling back to raw XML.
describe("useXmlEditorSession - initial mode for files with no Defs", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("defaults to form mode for a <Patch>-rooted file", async () => {
    invokeMock.mockResolvedValue(
      loadResult({ nodeCount: 1, rootElement: "Patch", profile: "patch", about: null, defs: [] }, "<Patch></Patch>"),
    );
    const { result } = renderHook(() => useXmlEditorSession("proj1", fileRef("Patches/A.xml")));

    await waitFor(() => expect(result.current?.loading).toBe(false));
    expect(result.current?.mode).toBe("form");
  });

  it("still defaults to raw mode for an empty <Defs>-rooted file", async () => {
    invokeMock.mockResolvedValue(
      loadResult({ nodeCount: 1, rootElement: "Defs", profile: "defs", about: null, defs: [] }, "<Defs></Defs>"),
    );
    const { result } = renderHook(() => useXmlEditorSession("proj1", fileRef("Defs/Empty.xml")));

    await waitFor(() => expect(result.current?.loading).toBe(false));
    expect(result.current?.mode).toBe("raw");
  });

  it("defaults to raw mode when the buffer fails to parse at all", async () => {
    invokeMock.mockResolvedValue(loadResult(null, "<Patch>"));
    const { result } = renderHook(() => useXmlEditorSession("proj1", fileRef("Patches/Broken.xml")));

    await waitFor(() => expect(result.current?.loading).toBe(false));
    expect(result.current?.mode).toBe("raw");
  });
});
