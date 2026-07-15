import { renderHook, waitFor, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { useXmlEditorSession } from "./useXmlEditorSession";
import type { XmlEditorFileRef } from "./useXmlEditorSession";
import type { XmlEditorDocumentView, XmlEdit } from "../types/xmlDocument";
import { formatError } from "../../../lib/formatError";

// These preconditions (document read-only or no active file, no Def selected, no active project)
// are known, enumerable conditions `useXmlEditorSession.ts` itself detects -- they must carry a
// structured `code` the shared renderer can translate (see `src/i18n/diagnostics.ts`), not only
// an English `Error.message`
// that bypasses localization entirely. Mirrors `useCustomFormViews.test.ts`'s equivalent
// assertion for the same bug class in a different feature.

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

function fileRef(overrides: Partial<XmlEditorFileRef> = {}): XmlEditorFileRef {
  return {
    locationId: "loc1",
    sourceKind: "project",
    readOnly: false,
    relativePath: "Defs/A.xml",
    ...overrides,
  };
}

describe("useXmlEditorSession - frontend-raised precondition errors", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("rejects insert/save calls with a structured code when the document is read-only", async () => {
    invokeMock.mockResolvedValue(
      loadResult(
        { nodeCount: 1, rootElement: "Defs", profile: "defs", about: null, defs: [] },
        "<Defs></Defs>",
      ),
    );
    const { result } = renderHook(() =>
      useXmlEditorSession("proj1", fileRef({ readOnly: true })),
    );
    await waitFor(() => expect(result.current?.loading).toBe(false));

    await expect(
      result.current!.insertDefFromTemplate("ThingDef", null, {}),
    ).rejects.toMatchObject({ code: "xml_editor_session_no_active_file" });
    await expect(
      result.current!.insertDefFromUserTemplate("tpl-1", "MyDef"),
    ).rejects.toMatchObject({ code: "xml_editor_session_no_active_file" });
    await expect(
      result.current!.saveSelectedDefAsTemplate("My template"),
    ).rejects.toMatchObject({ code: "xml_editor_session_no_active_file" });

    expect(invokeMock).toHaveBeenCalledTimes(1); // only the initial load
  });

  it("rejects saveSelectedDefAsTemplate with a structured code when no Def is selected", async () => {
    invokeMock.mockResolvedValue(
      loadResult(
        { nodeCount: 1, rootElement: "Defs", profile: "defs", about: null, defs: [] },
        "<Defs></Defs>",
      ),
    );
    const { result } = renderHook(() => useXmlEditorSession("proj1", fileRef()));
    await waitFor(() => expect(result.current?.loading).toBe(false));

    await expect(
      result.current!.saveSelectedDefAsTemplate("My template"),
    ).rejects.toMatchObject({ code: "xml_editor_session_no_def_selected" });
  });

  // Note: `deleteUserDefTemplate`'s own `!projectId` guard (raised via
  // `xmlEditorSessionErrors.ts`'s `noActiveProjectError`) is defense-in-depth only -- the hook
  // itself returns `null` whenever `!projectId || !relativePath` (see the final `if` before this
  // hook's `return` statement), so `deleteUserDefTemplate` is never actually reachable through the
  // public hook interface with no active project. Fixed for consistency/defense-in-depth anyway,
  // but there is no way to exercise it through `renderHook` the way the other two guards above
  // can be exercised (readOnly and "no Def selected" are both real, reachable states).

  // `apply_editor_edits` (src-tauri/src/services/xml_editor.rs) intentionally returns
  // `document: null` up front when the caller-supplied `raw_xml` is already fatally unparseable,
  // without attempting the edit -- reachable when a form field's debounced/queued commit lands
  // after the raw XML tab was left with unparseable text. The frontend's handling of that response
  // had been judged a can't-happen defensive case and left as a raw, untranslated `Error`; this
  // asserts it now carries a structured `code` that the shared renderer actually translates,
  // rather than falling back to the raw English message.
  it("rejects applyFormEdit with a structured, translated code when the backend returns no document", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "apply_xml_editor_edit") {
        return Promise.resolve(loadResult(null, "<Defs><ThingDef"));
      }
      return Promise.resolve(
        loadResult(
          { nodeCount: 1, rootElement: "Defs", profile: "defs", about: null, defs: [] },
          "<Defs></Defs>",
        ),
      );
    });
    const { result } = renderHook(() => useXmlEditorSession("proj1", fileRef()));
    await waitFor(() => expect(result.current?.loading).toBe(false));

    const edit: XmlEdit = {
      type: "setChildElementText",
      parentNodeId: 1,
      childName: "defName",
      value: "Foo",
    };

    let caught: unknown;
    await act(async () => {
      try {
        await result.current!.applyFormEdit(edit);
      } catch (e) {
        caught = e;
      }
    });

    expect(caught).toMatchObject({ code: "xml_editor_session_form_edit_no_document" });
    const rendered = formatError(caught);
    expect(rendered).not.toBe("Form edit returned no parsed document.");
    expect(rendered).toBe(
      "This change could not be applied because the document currently has a syntax error. Fix the XML and try again.",
    );
  });
});
