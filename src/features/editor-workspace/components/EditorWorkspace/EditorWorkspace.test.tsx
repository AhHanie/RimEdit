import { useState } from "react";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { confirm } from "@tauri-apps/plugin-dialog";
import type { ActiveEditorCommands, OpenFileTab } from "../../types";
import { EditorWorkspace } from "./EditorWorkspace";

// Replace CodeMirror with a plain textarea so workspace behavior tests use
// native DOM APIs for reading .value and firing change events.
vi.mock("../../../xml-editor/components/XmlRawEditor/XmlRawEditor", () => ({
  XmlRawEditor: ({
    value,
    onChange,
    readOnly,
  }: {
    value: string;
    onChange: (xml: string) => void;
    readOnly?: boolean;
    onShortcut?: unknown;
  }) => (
    <textarea
      aria-label="Raw XML editor"
      value={value}
      onChange={(e) => onChange(e.target.value)}
      readOnly={readOnly ?? false}
    />
  ),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);
const confirmMock = vi.mocked(confirm);

function makeTab(
  relativePath: string,
  fileName: string,
  folderPath: string,
): OpenFileTab {
  return {
    tabKey: `project-1:${relativePath}`,
    locationId: "project-1",
    sourceKind: "project",
    readOnly: false,
    relativePath,
    fileName,
    folderPath,
    dirty: false,
    editorKind: "xml",
  };
}

const initialTabs: OpenFileTab[] = [
  makeTab("Defs/A.xml", "A.xml", "Defs"),
  makeTab("Defs/B.xml", "B.xml", "Defs"),
];

function rawXmlFor(relativePath: string) {
  return `<Defs><ThingDef><defName>${relativePath}</defName></ThingDef></Defs>`;
}

function makeLoadResult(
  projectId: string,
  relativePath: string,
  rawXml = rawXmlFor(relativePath),
) {
  return {
    projectId,
    relativePath,
    rawXml,
    document: null,
    parseDiagnostics: [],
    validationDiagnostics: [],
  };
}

function getVisibleRawEditor() {
  const editor = screen
    .getAllByLabelText("Raw XML editor")
    .find((candidate) => !candidate.closest("[hidden]"));
  if (!editor) throw new Error("Expected a visible raw XML editor.");
  return editor as HTMLTextAreaElement;
}

async function findVisibleRawEditor() {
  await screen.findAllByLabelText("Raw XML editor");
  return getVisibleRawEditor();
}

function WorkspaceHarness({
  startingTabs = initialTabs,
  onActiveCommandsChange,
}: {
  startingTabs?: OpenFileTab[];
  onActiveCommandsChange?: (cmds: ActiveEditorCommands | null) => void;
}) {
  const [tabs, setTabs] = useState(startingTabs);
  const [activeTabKey, setActiveTabKey] = useState<string | null>(
    startingTabs[0]?.tabKey ?? null,
  );

  return (
    <EditorWorkspace
      tabs={tabs}
      activeTabKey={activeTabKey}
      projectId="project-1"
      catalog={null}
      onActivateTab={setActiveTabKey}
      onCloseTab={(tabKey) => {
        setTabs((prev) => prev.filter((tab) => tab.tabKey !== tabKey));
        setActiveTabKey((prev) => (prev === tabKey ? null : prev));
      }}
      onTabDirtyChange={(tabKey, dirty) => {
        setTabs((prev) => {
          const current = prev.find((tab) => tab.tabKey === tabKey);
          if (!current || current.dirty === dirty) return prev;
          return prev.map((tab) =>
            tab.tabKey === tabKey ? { ...tab, dirty } : tab,
          );
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
    const projectId = String((args as { projectId: string }).projectId);

    if (command === "read_project_xml_editor_document") {
      return Promise.resolve(makeLoadResult(projectId, relativePath));
    }

    if (command === "parse_xml_editor_buffer") {
      return Promise.resolve(
        makeLoadResult(
          projectId,
          relativePath,
          String((args as { rawXml: string }).rawXml),
        ),
      );
    }

    return Promise.reject(new Error(`Unexpected command: ${command}`));
  });
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("EditorWorkspace", () => {
  it("keeps raw XML edits mounted when switching tabs", async () => {
    render(<WorkspaceHarness />);

    const editedXml =
      "<Defs><ThingDef><defName>Edited A</defName></ThingDef></Defs>";
    const rawEditor = await findVisibleRawEditor();
    expect(rawEditor.value).toBe(rawXmlFor("Defs/A.xml"));

    fireEvent.change(rawEditor, { target: { value: editedXml } });
    expect(rawEditor.value).toBe(editedXml);

    fireEvent.click(screen.getByRole("button", { name: "B.xml" }));
    await waitFor(() => {
      expect(getVisibleRawEditor().value).toBe(rawXmlFor("Defs/B.xml"));
    });

    fireEvent.click(screen.getByRole("button", { name: "A.xml" }));
    expect(getVisibleRawEditor().value).toBe(editedXml);
  });

  it("shows a dirty marker for the edited tab", async () => {
    render(<WorkspaceHarness />);

    const rawEditor = await findVisibleRawEditor();
    fireEvent.change(rawEditor, {
      target: {
        value: "<Defs><ThingDef><defName>Dirty</defName></ThingDef></Defs>",
      },
    });

    const tablist = screen.getByRole("tablist");
    await waitFor(() => {
      expect(within(tablist).getAllByLabelText("Unsaved changes")).toHaveLength(
        1,
      );
    });
  });

  it("asks before closing a dirty tab", async () => {
    confirmMock.mockResolvedValueOnce(false);
    render(<WorkspaceHarness />);

    fireEvent.change(await findVisibleRawEditor(), {
      target: {
        value:
          "<Defs><ThingDef><defName>Dirty close</defName></ThingDef></Defs>",
      },
    });
    await waitFor(() => {
      expect(
        within(screen.getByRole("tablist")).getAllByLabelText(
          "Unsaved changes",
        ),
      ).toHaveLength(1);
    });

    fireEvent.click(screen.getByRole("button", { name: "Close A.xml" }));

    expect(confirmMock).toHaveBeenCalledWith(
      "Close this file and discard unsaved changes?",
      {
        title: "Discard unsaved changes?",
        kind: "warning",
        okLabel: "Discard",
        cancelLabel: "Cancel",
      },
    );
    expect(screen.getByRole("button", { name: "A.xml" })).not.toBeNull();

    confirmMock.mockResolvedValueOnce(true);
    fireEvent.click(screen.getByRole("button", { name: "Close A.xml" }));

    await waitFor(() => {
      expect(screen.queryByRole("button", { name: "A.xml" })).toBeNull();
    });
  });
});

// Helper: wait until onActiveCommandsChange publishes a non-null handle with canClose=true.
async function waitForCloseCmd(
  handler: ReturnType<typeof vi.fn>,
): Promise<ActiveEditorCommands> {
  let cmds: ActiveEditorCommands | null = null;
  await waitFor(() => {
    const latest = handler.mock.calls[handler.mock.calls.length - 1]?.[0] as
      | ActiveEditorCommands
      | null
      | undefined;
    if (!latest?.canClose) throw new Error("close command not yet ready");
    cmds = latest;
  });
  return cmds!;
}

describe("close command via onActiveCommandsChange", () => {
  it("closes a clean active tab when cmd.close() is called", async () => {
    const handleCmds = vi.fn();
    render(<WorkspaceHarness onActiveCommandsChange={handleCmds} />);

    const cmds = await waitForCloseCmd(handleCmds);
    await act(async () => {
      await cmds.close();
    });

    await waitFor(() => {
      expect(screen.queryByRole("button", { name: "A.xml" })).toBeNull();
    });
  });

  it("shows a confirmation dialog before closing a dirty tab", async () => {
    confirmMock.mockResolvedValueOnce(false);
    const handleCmds = vi.fn();
    render(<WorkspaceHarness onActiveCommandsChange={handleCmds} />);

    // Make the active tab dirty.
    fireEvent.change(await findVisibleRawEditor(), {
      target: {
        value: "<Defs><ThingDef><defName>Dirty</defName></ThingDef></Defs>",
      },
    });
    await waitFor(() => {
      expect(
        within(screen.getByRole("tablist")).getAllByLabelText(
          "Unsaved changes",
        ),
      ).toHaveLength(1);
    });

    const cmds = await waitForCloseCmd(handleCmds);
    await act(async () => {
      await cmds.close();
    });

    expect(confirmMock).toHaveBeenCalledWith(
      "Close this file and discard unsaved changes?",
      expect.any(Object),
    );
    // confirm returned false - tab must still be open.
    expect(screen.getByRole("button", { name: "A.xml" })).not.toBeNull();
  });

  it("keeps the tab open when the dirty-close confirmation is cancelled", async () => {
    confirmMock.mockResolvedValue(false);
    const handleCmds = vi.fn();
    render(<WorkspaceHarness onActiveCommandsChange={handleCmds} />);

    fireEvent.change(await findVisibleRawEditor(), {
      target: {
        value: "<Defs><ThingDef><defName>Dirty</defName></ThingDef></Defs>",
      },
    });
    await waitFor(() => {
      expect(
        within(screen.getByRole("tablist")).getAllByLabelText(
          "Unsaved changes",
        ),
      ).toHaveLength(1);
    });

    const cmds = await waitForCloseCmd(handleCmds);
    await act(async () => {
      await cmds.close();
    });

    expect(screen.getByRole("button", { name: "A.xml" })).not.toBeNull();
  });

  it("closes a dirty tab when the confirmation is accepted", async () => {
    confirmMock.mockResolvedValue(true);
    const handleCmds = vi.fn();
    render(<WorkspaceHarness onActiveCommandsChange={handleCmds} />);

    fireEvent.change(await findVisibleRawEditor(), {
      target: {
        value: "<Defs><ThingDef><defName>Dirty</defName></ThingDef></Defs>",
      },
    });
    await waitFor(() => {
      expect(
        within(screen.getByRole("tablist")).getAllByLabelText(
          "Unsaved changes",
        ),
      ).toHaveLength(1);
    });

    const cmds = await waitForCloseCmd(handleCmds);
    await act(async () => {
      await cmds.close();
    });

    await waitFor(() => {
      expect(screen.queryByRole("button", { name: "A.xml" })).toBeNull();
    });
  });
});
