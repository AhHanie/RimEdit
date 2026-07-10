import React from "react";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { ProjectExplorerPanel } from "./ProjectExplorerPanel";
import type { FileTreeFolderNode, ProjectFileScan } from "../../types";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: vi.fn(),
}));

import { confirm } from "@tauri-apps/plugin-dialog";
const confirmMock = vi.mocked(confirm);

function defaultProps(
  overrides: Partial<Parameters<typeof ProjectExplorerPanel>[0]> = {},
) {
  return {
    visible: true,
    scan: null,
    fileTree: null,
    activeFilePath: null,
    loadingScan: false,
    refreshingScan: false,
    hasActiveProject: false,
    searchQuery: "",
    filteredFiles: [],
    expandedFolders: new Set<string>(),
    onSearchChange: vi.fn(),
    onToggleFolder: vi.fn(),
    onSelectFile: vi.fn(),
    onSelectFilePath: vi.fn(),
    onRefresh: vi.fn(),
    onOpenProject: vi.fn(),
    onAddSourceFolder: vi.fn(),
    searchInputRef: { current: null } as React.RefObject<HTMLInputElement | null>,
    mutationError: null,
    onClearMutationError: vi.fn(),
    onCreateFile: vi.fn(),
    onCreateAndOpenFile: vi.fn(),
    onCreateFolder: vi.fn(),
    onRename: vi.fn(),
    onDelete: vi.fn(),
    ...overrides,
  };
}

const emptyScan: ProjectFileScan = {
  projectId: "proj1",
  projectRoot: "/tmp/proj",
  folders: [],
  files: [],
};

describe("ProjectExplorerPanel empty state", () => {
  it("shows both Open Project and Add Source Folder when no project is active", () => {
    render(<ProjectExplorerPanel {...defaultProps()} />);
    expect(screen.getByRole("button", { name: "Open Project" })).toBeDefined();
    expect(screen.getByRole("button", { name: "Add Source Folder" })).toBeDefined();
  });

  it("calls onAddSourceFolder when Add Source Folder is clicked", () => {
    const onAddSourceFolder = vi.fn();
    render(<ProjectExplorerPanel {...defaultProps({ onAddSourceFolder })} />);
    fireEvent.click(screen.getByRole("button", { name: "Add Source Folder" }));
    expect(onAddSourceFolder).toHaveBeenCalledOnce();
  });

  it("calls onOpenProject when Open Project is clicked", () => {
    const onOpenProject = vi.fn();
    render(<ProjectExplorerPanel {...defaultProps({ onOpenProject })} />);
    fireEvent.click(screen.getByRole("button", { name: "Open Project" }));
    expect(onOpenProject).toHaveBeenCalledOnce();
  });

  it("renders the project root row when the project has no files or folders", () => {
    const emptyFileTree: FileTreeFolderNode = {
      type: "folder",
      id: "",
      name: "Project",
      relativePath: "",
      children: [],
    };
    render(
      <ProjectExplorerPanel
        {...defaultProps({
          hasActiveProject: true,
          scan: emptyScan,
          fileTree: emptyFileTree,
          expandedFolders: new Set([""]),
        })}
      />,
    );
    expect(screen.getByText("Project")).toBeDefined();
  });

  it("shows 'No matching files' when search returns no results", () => {
    render(
      <ProjectExplorerPanel
        {...defaultProps({
          hasActiveProject: true,
          scan: emptyScan,
          searchQuery: "xyz",
          filteredFiles: [],
        })}
      />,
    );
    expect(screen.getByText("No matching files")).toBeDefined();
  });
});

describe("ProjectExplorerPanel empty project root context menu", () => {
  const emptyFileTree: FileTreeFolderNode = {
    type: "folder",
    id: "",
    name: "Project",
    relativePath: "",
    children: [],
  };
  const emptyTreeProps = {
    hasActiveProject: true,
    scan: emptyScan,
    fileTree: emptyFileTree,
    expandedFolders: new Set([""]),
  };

  it("right-clicking the empty-project root row shows only create actions", () => {
    render(<ProjectExplorerPanel {...defaultProps(emptyTreeProps)} />);
    const rootRow = screen.getByText("Project").closest('[role="treeitem"]')!;
    fireEvent.contextMenu(rootRow);
    const menu = screen.getByRole("menu");
    const items = menu.querySelectorAll('[role="menuitem"]');
    const labels = Array.from(items).map((el) => el.textContent);
    expect(labels).toContain("New File");
    expect(labels).toContain("New Folder");
    expect(labels).toContain("New Defs File");
    expect(labels).toContain("New Patches File");
    expect(labels).not.toContain("Rename");
    expect(labels).not.toContain("Delete");
  });

  it("committing New Folder from the empty-project root calls onCreateFolder with an empty parent path", async () => {
    const onCreateFolder = vi.fn().mockResolvedValue(undefined);
    render(<ProjectExplorerPanel {...defaultProps({ ...emptyTreeProps, onCreateFolder })} />);
    const rootRow = screen.getByText("Project").closest('[role="treeitem"]')!;
    fireEvent.contextMenu(rootRow);
    fireEvent.click(screen.getByRole("menuitem", { name: "New Folder" }));
    const input = screen.getByRole("textbox") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "FolderName" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(onCreateFolder).toHaveBeenCalledWith("", "FolderName"));
  });

  it("committing New File from the empty-project root calls onCreateFile with an empty parent path", async () => {
    const onCreateFile = vi.fn().mockResolvedValue(undefined);
    render(<ProjectExplorerPanel {...defaultProps({ ...emptyTreeProps, onCreateFile })} />);
    const rootRow = screen.getByText("Project").closest('[role="treeitem"]')!;
    fireEvent.contextMenu(rootRow);
    fireEvent.click(screen.getByRole("menuitem", { name: "New File" }));
    const input = screen.getByRole("textbox") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "FileName.ext" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(onCreateFile).toHaveBeenCalledWith("", "FileName.ext"));
  });

  it("right-clicking blank tree space opens the root create menu", () => {
    render(<ProjectExplorerPanel {...defaultProps(emptyTreeProps)} />);
    const tree = screen.getByRole("tree", { name: "Project files" });
    fireEvent.contextMenu(tree);
    const menu = screen.getByRole("menu");
    const items = menu.querySelectorAll('[role="menuitem"]');
    const labels = Array.from(items).map((el) => el.textContent);
    expect(labels).toContain("New File");
    expect(labels).toContain("New Folder");
    expect(labels).toContain("New Defs File");
    expect(labels).toContain("New Patches File");
    expect(labels).not.toContain("Rename");
    expect(labels).not.toContain("Delete");
  });
});

describe("ProjectExplorerPanel search results", () => {
  it("shows non-XML files in search results", () => {
    const filteredFiles = [
      {
        relativePath: "notes.txt",
        folderPath: "",
        fileName: "notes.txt",
        extension: "txt",
        sizeBytes: 10,
        fileKind: "text" as const,
      },
    ];
    render(
      <ProjectExplorerPanel
        {...defaultProps({
          hasActiveProject: true,
          scan: { projectId: "proj1", projectRoot: "/tmp", folders: [], files: filteredFiles },
          searchQuery: "notes",
          filteredFiles,
        })}
      />,
    );
    expect(screen.getByText("notes.txt")).toBeDefined();
  });
});

const minimalScan: ProjectFileScan = {
  projectId: "proj1",
  projectRoot: "/tmp/proj",
  folders: [{ relativePath: "Defs", folderName: "Defs", parentPath: "" }],
  files: [
    {
      relativePath: "Defs/Things.xml",
      folderPath: "Defs",
      fileName: "Things.xml",
      extension: "xml",
      sizeBytes: 100,
      fileKind: "xml",
    },
  ],
};

const minimalFileTree: FileTreeFolderNode = {
  type: "folder",
  id: "",
  name: "Project",
  relativePath: "",
  children: [
    {
      type: "folder",
      id: "Defs",
      name: "Defs",
      relativePath: "Defs",
      children: [
        {
          type: "file",
          id: "Defs/Things.xml",
          name: "Things.xml",
          relativePath: "Defs/Things.xml",
          folderPath: "Defs",
          extension: "xml",
          sizeBytes: 100,
          fileKind: "xml",
        },
      ],
    },
  ],
};

describe("ProjectExplorerPanel refreshingScan", () => {
  it("shows scanning state when loadingScan is true and scan is null", () => {
    render(
      <ProjectExplorerPanel
        {...defaultProps({ hasActiveProject: true, loadingScan: true, scan: null })}
      />,
    );
    expect(screen.getByText("Scanning project…")).toBeDefined();
  });

  it("keeps tree rendered and does not show scanning state when refreshingScan is true", () => {
    render(
      <ProjectExplorerPanel
        {...defaultProps({
          hasActiveProject: true,
          refreshingScan: true,
          scan: minimalScan,
          fileTree: minimalFileTree,
          expandedFolders: new Set(["", "Defs"]),
        })}
      />,
    );
    expect(screen.queryByText("Scanning project…")).toBeNull();
    expect(screen.getByText("Defs")).toBeDefined();
  });

  it("disables the refresh button when refreshingScan is true", () => {
    render(
      <ProjectExplorerPanel
        {...defaultProps({
          hasActiveProject: true,
          refreshingScan: true,
          scan: minimalScan,
          fileTree: minimalFileTree,
          expandedFolders: new Set([""]),
        })}
      />,
    );
    const refreshBtn = screen.getByRole("button", { name: "Refresh files" });
    expect((refreshBtn as HTMLButtonElement).disabled).toBe(true);
  });
});

describe("context menu and delete confirmation", () => {
  const treeProps = {
    hasActiveProject: true,
    scan: minimalScan,
    fileTree: minimalFileTree,
    expandedFolders: new Set(["", "Defs"]),
  };

  beforeEach(() => {
    confirmMock.mockReset();
  });

  it("right-clicking a file shows only Rename and Delete", () => {
    render(<ProjectExplorerPanel {...defaultProps(treeProps)} />);
    const fileRow = screen.getByTitle("Defs/Things.xml");
    fireEvent.contextMenu(fileRow);
    const menu = screen.getByRole("menu");
    const items = menu.querySelectorAll('[role="menuitem"]');
    const labels = Array.from(items).map((el) => el.textContent);
    expect(labels).toEqual(["Rename", "Delete"]);
  });

  it("right-clicking a non-root folder shows New File, New Folder, New Defs File, New Patches File, Rename, Delete", () => {
    render(<ProjectExplorerPanel {...defaultProps(treeProps)} />);
    const folderRow = screen.getByText("Defs").closest('[role="treeitem"]')!;
    fireEvent.contextMenu(folderRow);
    const menu = screen.getByRole("menu");
    const items = menu.querySelectorAll('[role="menuitem"]');
    const labels = Array.from(items).map((el) => el.textContent);
    expect(labels).toContain("New File");
    expect(labels).toContain("New Folder");
    expect(labels).toContain("New Defs File");
    expect(labels).toContain("New Patches File");
    expect(labels).toContain("Rename");
    expect(labels).toContain("Delete");
  });

  it("right-clicking the root folder shows only the New * actions", () => {
    render(<ProjectExplorerPanel {...defaultProps(treeProps)} />);
    const rootRow = screen.getByText("Project").closest('[role="treeitem"]')!;
    fireEvent.contextMenu(rootRow);
    const menu = screen.getByRole("menu");
    const items = menu.querySelectorAll('[role="menuitem"]');
    const labels = Array.from(items).map((el) => el.textContent);
    expect(labels).toContain("New File");
    expect(labels).toContain("New Folder");
    expect(labels).toContain("New Defs File");
    expect(labels).toContain("New Patches File");
    expect(labels).not.toContain("Rename");
    expect(labels).not.toContain("Delete");
  });

  it("right-clicking blank space with existing child rows still targets root, not the child", () => {
    render(<ProjectExplorerPanel {...defaultProps(treeProps)} />);
    const tree = screen.getByRole("tree", { name: "Project files" });
    fireEvent.contextMenu(tree);
    const menu = screen.getByRole("menu");
    const items = menu.querySelectorAll('[role="menuitem"]');
    const labels = Array.from(items).map((el) => el.textContent);
    expect(labels).toContain("New File");
    expect(labels).toContain("New Folder");
    expect(labels).not.toContain("Rename");
    expect(labels).not.toContain("Delete");
  });

  it("clicking Rename from file context menu shows inline rename input", () => {
    render(<ProjectExplorerPanel {...defaultProps(treeProps)} />);
    const fileRow = screen.getByTitle("Defs/Things.xml");
    fireEvent.contextMenu(fileRow);
    fireEvent.click(screen.getByRole("menuitem", { name: "Rename" }));
    expect(screen.queryByRole("menu")).toBeNull();
    const input = screen.getByRole("textbox");
    expect((input as HTMLInputElement).value).toBe("Things.xml");
  });

  it("clicking New File from folder context menu shows inline creation input", () => {
    render(<ProjectExplorerPanel {...defaultProps(treeProps)} />);
    const folderRow = screen.getByText("Defs").closest('[role="treeitem"]')!;
    fireEvent.contextMenu(folderRow);
    fireEvent.click(screen.getByRole("menuitem", { name: "New File" }));
    expect(screen.queryByRole("menu")).toBeNull();
    const input = screen.getByRole("textbox");
    expect((input as HTMLInputElement).value).toBe("");
  });

  it("clicking New Defs File pre-fills a default name that the user can edit before committing", () => {
    render(<ProjectExplorerPanel {...defaultProps(treeProps)} />);
    const folderRow = screen.getByText("Defs").closest('[role="treeitem"]')!;
    fireEvent.contextMenu(folderRow);
    fireEvent.click(screen.getByRole("menuitem", { name: "New Defs File" }));
    expect(screen.queryByRole("menu")).toBeNull();
    const input = screen.getByRole("textbox") as HTMLInputElement;
    expect(input.value).toBe("NewDefs.xml");
  });

  it("committing New File creates an empty file via onCreateFile, not onCreateAndOpenFile", async () => {
    const onCreateFile = vi.fn().mockResolvedValue(undefined);
    const onCreateAndOpenFile = vi.fn().mockResolvedValue(undefined);
    render(
      <ProjectExplorerPanel
        {...defaultProps({ ...treeProps, onCreateFile, onCreateAndOpenFile })}
      />,
    );
    const folderRow = screen.getByText("Defs").closest('[role="treeitem"]')!;
    fireEvent.contextMenu(folderRow);
    fireEvent.click(screen.getByRole("menuitem", { name: "New File" }));
    const input = screen.getByRole("textbox") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "Generic.xml" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(onCreateFile).toHaveBeenCalledOnce());
    expect(onCreateFile).toHaveBeenCalledWith("Defs", "Generic.xml");
    expect(onCreateAndOpenFile).not.toHaveBeenCalled();
  });

  it("committing New Defs File calls onCreateAndOpenFile with the Defs template", async () => {
    const onCreateAndOpenFile = vi.fn().mockResolvedValue(undefined);
    render(<ProjectExplorerPanel {...defaultProps({ ...treeProps, onCreateAndOpenFile })} />);
    const folderRow = screen.getByText("Defs").closest('[role="treeitem"]')!;
    fireEvent.contextMenu(folderRow);
    fireEvent.click(screen.getByRole("menuitem", { name: "New Defs File" }));
    const input = screen.getByRole("textbox") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "Buildings.xml" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(onCreateAndOpenFile).toHaveBeenCalledOnce());
    expect(onCreateAndOpenFile).toHaveBeenCalledWith(
      "Defs",
      "Buildings.xml",
      expect.stringContaining("<Defs>"),
    );
  });

  it("committing New Patches File with a retyped name missing .xml still enforces the extension", async () => {
    const onCreateAndOpenFile = vi.fn().mockResolvedValue(undefined);
    render(<ProjectExplorerPanel {...defaultProps({ ...treeProps, onCreateAndOpenFile })} />);
    const folderRow = screen.getByText("Defs").closest('[role="treeitem"]')!;
    fireEvent.contextMenu(folderRow);
    fireEvent.click(screen.getByRole("menuitem", { name: "New Patches File" }));
    const input = screen.getByRole("textbox") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "MyPatch" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(onCreateAndOpenFile).toHaveBeenCalledOnce());
    expect(onCreateAndOpenFile).toHaveBeenCalledWith(
      "Defs",
      "MyPatch.xml",
      expect.stringContaining("<Patch>"),
    );
  });

  it("committing New Patches File calls onCreateAndOpenFile with the Patch template", async () => {
    const onCreateAndOpenFile = vi.fn().mockResolvedValue(undefined);
    render(<ProjectExplorerPanel {...defaultProps({ ...treeProps, onCreateAndOpenFile })} />);
    const folderRow = screen.getByText("Defs").closest('[role="treeitem"]')!;
    fireEvent.contextMenu(folderRow);
    fireEvent.click(screen.getByRole("menuitem", { name: "New Patches File" }));
    const input = screen.getByRole("textbox") as HTMLInputElement;
    expect(input.value).toBe("NewPatches.xml");
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(onCreateAndOpenFile).toHaveBeenCalledOnce());
    expect(onCreateAndOpenFile).toHaveBeenCalledWith(
      "Defs",
      "NewPatches.xml",
      expect.stringContaining("<Patch>"),
    );
  });

  it("New Defs File default name avoids colliding with an existing sibling", () => {
    const scanWithNewDefs: ProjectFileScan = {
      ...minimalScan,
      files: [
        ...minimalScan.files,
        {
          relativePath: "Defs/NewDefs.xml",
          folderPath: "Defs",
          fileName: "NewDefs.xml",
          extension: "xml",
          sizeBytes: 10,
          fileKind: "xml",
        },
      ],
    };
    const treeWithNewDefs: FileTreeFolderNode = {
      ...minimalFileTree,
      children: [
        {
          ...(minimalFileTree.children[0] as FileTreeFolderNode),
          children: [
            ...(minimalFileTree.children[0] as FileTreeFolderNode).children,
            {
              type: "file",
              id: "Defs/NewDefs.xml",
              name: "NewDefs.xml",
              relativePath: "Defs/NewDefs.xml",
              folderPath: "Defs",
              extension: "xml",
              sizeBytes: 10,
              fileKind: "xml",
            },
          ],
        },
      ],
    };
    render(
      <ProjectExplorerPanel
        {...defaultProps({
          ...treeProps,
          scan: scanWithNewDefs,
          fileTree: treeWithNewDefs,
        })}
      />,
    );
    const folderRow = screen.getByText("Defs").closest('[role="treeitem"]')!;
    fireEvent.contextMenu(folderRow);
    fireEvent.click(screen.getByRole("menuitem", { name: "New Defs File" }));
    const input = screen.getByRole("textbox") as HTMLInputElement;
    expect(input.value).toBe("NewDefs1.xml");
  });

  it("clicking Delete opens Tauri confirm dialog before calling onDelete", async () => {
    confirmMock.mockResolvedValue(false);
    const onDelete = vi.fn();
    render(<ProjectExplorerPanel {...defaultProps({ ...treeProps, onDelete })} />);
    const fileRow = screen.getByTitle("Defs/Things.xml");
    fireEvent.contextMenu(fileRow);
    fireEvent.click(screen.getByRole("menuitem", { name: "Delete" }));
    await waitFor(() => expect(confirmMock).toHaveBeenCalledOnce());
    expect(onDelete).not.toHaveBeenCalled();
  });

  it("onDelete is called when confirm resolves true", async () => {
    confirmMock.mockResolvedValue(true);
    const onDelete = vi.fn().mockResolvedValue(undefined);
    render(<ProjectExplorerPanel {...defaultProps({ ...treeProps, onDelete })} />);
    const fileRow = screen.getByTitle("Defs/Things.xml");
    fireEvent.contextMenu(fileRow);
    fireEvent.click(screen.getByRole("menuitem", { name: "Delete" }));
    await waitFor(() => expect(onDelete).toHaveBeenCalledWith("Defs/Things.xml", "file"));
  });

  it("onDelete is not called when confirm resolves false", async () => {
    confirmMock.mockResolvedValue(false);
    const onDelete = vi.fn();
    render(<ProjectExplorerPanel {...defaultProps({ ...treeProps, onDelete })} />);
    const fileRow = screen.getByTitle("Defs/Things.xml");
    fireEvent.contextMenu(fileRow);
    fireEvent.click(screen.getByRole("menuitem", { name: "Delete" }));
    await waitFor(() => expect(confirmMock).toHaveBeenCalledOnce());
    expect(onDelete).not.toHaveBeenCalled();
  });
});
