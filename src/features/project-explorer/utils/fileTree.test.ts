import { describe, it, expect } from "vitest";
import { buildFileTree, collectFolderIds } from "./fileTree";
import type { ProjectFileScan } from "../types";

function makeScan(overrides: Partial<ProjectFileScan> = {}): ProjectFileScan {
  return {
    projectId: "proj1",
    projectRoot: "/tmp/proj",
    folders: [],
    files: [],
    ...overrides,
  };
}

describe("buildFileTree", () => {
  it("includes XML files", () => {
    const scan = makeScan({
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
    });
    const tree = buildFileTree(scan, "Project");
    const defs = tree.children.find((c) => c.type === "folder" && c.name === "Defs");
    expect(defs).toBeDefined();
    expect(defs?.type === "folder" && defs.children[0]?.name).toBe("Things.xml");
  });

  it("includes non-XML files (txt, png, extensionless)", () => {
    const scan = makeScan({
      files: [
        {
          relativePath: "notes.txt",
          folderPath: "",
          fileName: "notes.txt",
          extension: "txt",
          sizeBytes: 10,
          fileKind: "text",
        },
        {
          relativePath: "icon.png",
          folderPath: "",
          fileName: "icon.png",
          extension: "png",
          sizeBytes: 512,
          fileKind: "binary",
        },
        {
          relativePath: "noext",
          folderPath: "",
          fileName: "noext",
          extension: "",
          sizeBytes: 5,
          fileKind: "unknown",
        },
      ],
    });
    const tree = buildFileTree(scan, "Project");
    const names = tree.children.map((c) => c.name);
    expect(names).toContain("notes.txt");
    expect(names).toContain("icon.png");
    expect(names).toContain("noext");
  });

  it("includes empty folders from scan.folders", () => {
    const scan = makeScan({
      folders: [{ relativePath: "EmptyDir", folderName: "EmptyDir", parentPath: "" }],
      files: [],
    });
    const tree = buildFileTree(scan, "Project");
    const emptyDir = tree.children.find((c) => c.name === "EmptyDir");
    expect(emptyDir).toBeDefined();
    expect(emptyDir?.type).toBe("folder");
    if (emptyDir?.type === "folder") {
      expect(emptyDir.children).toHaveLength(0);
    }
  });

  it("includes folders containing only non-XML files", () => {
    const scan = makeScan({
      folders: [{ relativePath: "Assets", folderName: "Assets", parentPath: "" }],
      files: [
        {
          relativePath: "Assets/icon.png",
          folderPath: "Assets",
          fileName: "icon.png",
          extension: "png",
          sizeBytes: 512,
          fileKind: "binary",
        },
      ],
    });
    const tree = buildFileTree(scan, "Project");
    const assets = tree.children.find((c) => c.name === "Assets");
    expect(assets).toBeDefined();
    expect(assets?.type).toBe("folder");
    if (assets?.type === "folder") {
      expect(assets.children).toHaveLength(1);
      expect(assets.children[0]?.name).toBe("icon.png");
    }
  });

  it("passes fileKind through to file nodes", () => {
    const scan = makeScan({
      files: [
        {
          relativePath: "notes.txt",
          folderPath: "",
          fileName: "notes.txt",
          extension: "txt",
          sizeBytes: 10,
          fileKind: "text",
        },
      ],
    });
    const tree = buildFileTree(scan, "Project");
    const node = tree.children[0];
    expect(node?.type).toBe("file");
    if (node?.type === "file") {
      expect(node.fileKind).toBe("text");
    }
  });

  it("sorts folders before files and names case-insensitively", () => {
    const scan = makeScan({
      folders: [{ relativePath: "Zebra", folderName: "Zebra", parentPath: "" }],
      files: [
        {
          relativePath: "apple.xml",
          folderPath: "",
          fileName: "apple.xml",
          extension: "xml",
          sizeBytes: 10,
          fileKind: "xml",
        },
      ],
    });
    const tree = buildFileTree(scan, "Project");
    expect(tree.children[0]?.type).toBe("folder");
    expect(tree.children[1]?.type).toBe("file");
  });
});

describe("collectFolderIds", () => {
  it("includes the root id", () => {
    const scan = makeScan();
    const tree = buildFileTree(scan, "Project");
    const ids = collectFolderIds(tree);
    expect(ids.has("")).toBe(true);
  });

  it("includes nested folder ids at all levels", () => {
    const scan = makeScan({
      folders: [
        { relativePath: "Defs", folderName: "Defs", parentPath: "" },
        { relativePath: "Defs/Items", folderName: "Items", parentPath: "Defs" },
      ],
    });
    const tree = buildFileTree(scan, "Project");
    const ids = collectFolderIds(tree);
    expect(ids.has("")).toBe(true);
    expect(ids.has("Defs")).toBe(true);
    expect(ids.has("Defs/Items")).toBe(true);
  });

  it("does not include file node ids", () => {
    const scan = makeScan({
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
    });
    const tree = buildFileTree(scan, "Project");
    const ids = collectFolderIds(tree);
    expect(ids.has("Defs/Things.xml")).toBe(false);
  });
});
