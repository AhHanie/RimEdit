import { FALLBACK_LOCALE } from "../../../i18n/locale";
import type {
  FileTreeFileNode,
  FileTreeFolderNode,
  ProjectFileEntry,
  ProjectFileScan,
} from "../types";

export function buildFileTree(
  scan: ProjectFileScan,
  rootName: string,
  locale: string = FALLBACK_LOCALE,
): FileTreeFolderNode {
  const folderMap = new Map<string, FileTreeFolderNode>();

  const root: FileTreeFolderNode = {
    type: "folder",
    id: "",
    relativePath: "",
    name: rootName,
    children: [],
  };
  folderMap.set("", root);

  function ensureFolder(folderPath: string): FileTreeFolderNode {
    if (folderPath === "") return root;

    const existing = folderMap.get(folderPath);
    if (existing) return existing;

    const lastSlash = folderPath.lastIndexOf("/");
    const name = lastSlash === -1 ? folderPath : folderPath.slice(lastSlash + 1);
    const parentPath = lastSlash === -1 ? "" : folderPath.slice(0, lastSlash);

    const parent = ensureFolder(parentPath);
    const node: FileTreeFolderNode = {
      type: "folder",
      id: folderPath,
      relativePath: folderPath,
      name,
      children: [],
    };
    parent.children.push(node);
    folderMap.set(folderPath, node);
    return node;
  }

  // Insert explicit folders first so empty folders appear
  for (const folder of scan.folders) {
    ensureFolder(folder.relativePath);
  }

  for (const entry of scan.files) {
    const parent = ensureFolder(entry.folderPath);
    const fileNode: FileTreeFileNode = {
      type: "file",
      id: entry.relativePath,
      name: entry.fileName,
      relativePath: entry.relativePath,
      folderPath: entry.folderPath,
      extension: entry.extension,
      sizeBytes: entry.sizeBytes,
      fileKind: entry.fileKind,
      activeForGameVersion: entry.activeForGameVersion,
    };
    parent.children.push(fileNode);
  }

  function sortChildren(node: FileTreeFolderNode): void {
    node.children.sort((a, b) => {
      if (a.type !== b.type) return a.type === "folder" ? -1 : 1;
      return a.name.localeCompare(b.name, locale, { sensitivity: "base" });
    });
    for (const child of node.children) {
      if (child.type === "folder") sortChildren(child);
    }
  }

  sortChildren(root);
  return root;
}

export function filterFiles(files: ProjectFileEntry[], query: string): ProjectFileEntry[] {
  const q = query.trim().toLowerCase();
  if (!q) return files;
  return files.filter(
    (f) =>
      f.fileName.toLowerCase().includes(q) ||
      f.relativePath.toLowerCase().includes(q) ||
      f.folderPath.toLowerCase().includes(q),
  );
}

export function countTreeFiles(node: FileTreeFolderNode): number {
  return node.children.reduce<number>((acc, child) => {
    return acc + (child.type === "file" ? 1 : countTreeFiles(child));
  }, 0);
}

export function getImmediateChildFolderIds(node: FileTreeFolderNode): string[] {
  return node.children
    .filter((c): c is FileTreeFolderNode => c.type === "folder")
    .map((c) => c.id);
}

export function collectFolderIds(node: FileTreeFolderNode): Set<string> {
  const ids = new Set<string>();
  function collect(n: FileTreeFolderNode) {
    ids.add(n.id);
    for (const child of n.children) {
      if (child.type === "folder") collect(child);
    }
  }
  collect(node);
  return ids;
}
