/**
 * Integration tests using the real XmlRawEditor (CodeMirror) rather than the
 * textarea mock used in EditorWorkspace.test.tsx.
 *
 * These tests verify that CodeMirror's internal transactions do not bubble up
 * as spurious onChange / updateRawXml calls, which would corrupt session state
 * (clearing redo history, triggering unnecessary parse debounces, etc.).
 */

import { useState } from "react";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { confirm } from "@tauri-apps/plugin-dialog";
import type { ActiveEditorCommands, OpenFileTab } from "../../types";
import { EditorWorkspace } from "./EditorWorkspace";

// CodeMirror uses ResizeObserver for viewport tracking; polyfill for JSDOM.
beforeAll(() => {
  if (!globalThis.ResizeObserver) {
    globalThis.ResizeObserver = class ResizeObserver {
      observe() {}
      unobserve() {}
      disconnect() {}
    };
  }
});

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ confirm: vi.fn() }));

const invokeMock = vi.mocked(invoke);
const confirmMock = vi.mocked(confirm);

function makeTab(relativePath: string, fileName: string): OpenFileTab {
  return {
    tabKey: `project-1:${relativePath}`,
    locationId: "project-1",
    sourceKind: "project",
    readOnly: false,
    relativePath,
    fileName,
    folderPath: "Defs",
    dirty: false,
    editorKind: "xml",
  };
}

const initialTabs: OpenFileTab[] = [
  makeTab("Defs/A.xml", "A.xml"),
  makeTab("Defs/B.xml", "B.xml"),
];

function rawXmlFor(relativePath: string) {
  return `<Defs><ThingDef><defName>${relativePath}</defName></ThingDef></Defs>`;
}

function makeLoadResult(relativePath: string) {
  return {
    projectId: "project-1",
    relativePath,
    rawXml: rawXmlFor(relativePath),
    document: null,
    parseDiagnostics: [],
    validationDiagnostics: [],
  };
}

function WorkspaceHarness({
  onActiveCommandsChange,
}: {
  onActiveCommandsChange?: (cmds: ActiveEditorCommands | null) => void;
} = {}) {
  const [tabs, setTabs] = useState(initialTabs);
  const [activeTabKey, setActiveTabKey] = useState<string | null>(
    initialTabs[0]?.tabKey ?? null,
  );

  return (
    <EditorWorkspace
      tabs={tabs}
      activeTabKey={activeTabKey}
      projectId="project-1"
      catalog={null}
      onActivateTab={setActiveTabKey}
      onCloseTab={(tabKey) => {
        setTabs((prev) => prev.filter((t) => t.tabKey !== tabKey));
        setActiveTabKey((prev) => (prev === tabKey ? null : prev));
      }}
      onTabDirtyChange={(tabKey, dirty) => {
        setTabs((prev) => {
          const current = prev.find((t) => t.tabKey === tabKey);
          if (!current || current.dirty === dirty) return prev;
          return prev.map((t) => (t.tabKey === tabKey ? { ...t, dirty } : t));
        });
      }}
      onActiveCommandsChange={onActiveCommandsChange}
    />
  );
}

beforeEach(() => {
  confirmMock.mockResolvedValue(true);
  invokeMock.mockImplementation((command, args) => {
    const relativePath = String(
      (args as { relativePath: string }).relativePath,
    );

    if (command === "read_project_xml_editor_document") {
      return Promise.resolve(makeLoadResult(relativePath));
    }
    if (command === "parse_xml_editor_buffer") {
      return Promise.resolve(
        makeLoadResult(String((args as { relativePath: string }).relativePath)),
      );
    }

    return Promise.reject(new Error(`Unexpected command: ${command}`));
  });
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("EditorWorkspace with real CodeMirror", () => {
  it("loads files into CodeMirror without triggering spurious parse calls", async () => {
    render(<WorkspaceHarness />);

    // Wait for tab A's CodeMirror to show its content.
    await waitFor(() => {
      const editors = screen
        .queryAllByLabelText("Raw XML editor")
        .filter((el) => !el.closest("[hidden]"));
      expect(editors.length).toBeGreaterThan(0);
    });

    // Allow effects and the 300ms parse debounce to settle.
    await new Promise((r) => setTimeout(r, 400));

    // No spurious parse calls - only the initial read_project_xml_editor_document
    // should have fired, not parse_xml_editor_buffer.
    const parseCalls = invokeMock.mock.calls.filter(
      ([cmd]) => cmd === "parse_xml_editor_buffer",
    );
    expect(parseCalls).toHaveLength(0);
  });

  // Helper: wait until onActiveCommandsChange publishes a non-null handle with canClose=true.
  async function waitForCommands(
    handler: ReturnType<typeof vi.fn>,
  ): Promise<ActiveEditorCommands> {
    let cmds: ActiveEditorCommands | null = null;
    await waitFor(() => {
      const latest = handler.mock.calls[handler.mock.calls.length - 1]?.[0] as
        | ActiveEditorCommands
        | null
        | undefined;
      if (!latest?.canClose) throw new Error("commands not yet ready");
      cmds = latest;
    });
    return cmds!;
  }

  it("publishes canClose=true and canUndo=false after the session loads", async () => {
    const handleCmds = vi.fn();
    render(<WorkspaceHarness onActiveCommandsChange={handleCmds} />);

    const cmds = await waitForCommands(handleCmds);
    expect(cmds.canClose).toBe(true);
    expect(cmds.canUndo).toBe(false);
    expect(cmds.canRedo).toBe(false);
  });

  it("cmd.close() removes the active tab end-to-end with real CodeMirror mounted", async () => {
    const handleCmds = vi.fn();
    render(<WorkspaceHarness onActiveCommandsChange={handleCmds} />);

    // Wait for the raw editor to render.
    await waitFor(() => {
      const editors = screen
        .queryAllByLabelText("Raw XML editor")
        .filter((el) => !el.closest("[hidden]"));
      expect(editors.length).toBeGreaterThan(0);
    });

    const cmds = await waitForCommands(handleCmds);
    await act(async () => {
      await cmds.close();
    });

    await waitFor(() => {
      expect(screen.queryByRole("button", { name: "A.xml" })).toBeNull();
    });
  });

  it("Ctrl+Z in raw mode calls onShortcut and does not dirty the tab via CodeMirror internal undo", async () => {
    const handleCmds = vi.fn();
    render(<WorkspaceHarness onActiveCommandsChange={handleCmds} />);

    const editor = await waitFor(() => {
      const editors = screen
        .queryAllByLabelText("Raw XML editor")
        .filter((el) => !el.closest("[hidden]"));
      if (!editors.length) throw new Error("editor not mounted");
      return editors[0];
    });

    // Find the outer CodeMirror container so the keydown reaches its handler.
    const cmEditorEl = editor.closest(".cm-editor");
    expect(cmEditorEl).not.toBeNull();

    // Allow the initial load to settle fully.
    await waitForCommands(handleCmds);

    // Fire Ctrl+Z - with no prior edits the session has nothing to undo,
    // but the key must not trigger CodeMirror's own history (which would call
    // onChange and mark the tab dirty).
    fireEvent.keyDown(cmEditorEl!, { key: "z", ctrlKey: true });

    // Allow any async effects to flush.
    await new Promise((r) => setTimeout(r, 0));

    expect(screen.queryAllByLabelText("Unsaved changes")).toHaveLength(0);
  });

  it("does not dirty tabs when switching between loaded files", async () => {
    render(<WorkspaceHarness />);

    // Wait for tab A to load.
    await waitFor(() => {
      const editors = screen
        .queryAllByLabelText("Raw XML editor")
        .filter((el) => !el.closest("[hidden]"));
      expect(editors.length).toBeGreaterThan(0);
    });

    // Switch to B, then back to A.
    fireEvent.click(screen.getByRole("button", { name: "B.xml" }));
    fireEvent.click(screen.getByRole("button", { name: "A.xml" }));

    // Allow effects and the parse debounce to settle.
    await new Promise((r) => setTimeout(r, 400));

    // Neither tab should be dirty.
    expect(screen.queryAllByLabelText("Unsaved changes")).toHaveLength(0);
  });
});
